//! Essais des pixels rangés (`Tile`, `tuilepack.rs`) et de la tenue des dessins (`gel.rs`) : un dessin rangé puis rouvert est identique au
//! pixel près, un dessin modifié ne perd rien, l'annulation, l'enregistrement, la copie de secours et la relecture voient les mêmes pixels.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::document::{CelId, Document, Layer, LayerKind};
use crate::gel::Gel;
use crate::raster::{Tile, TILE_PIXELS};

fn pixel(i: usize) -> Color32 {
    if i % 64 == 17 || i / 64 == 40 {
        Color32::from_rgba_premultiplied(30, (i % 200) as u8, 90, 255)
    } else {
        Color32::TRANSPARENT
    }
}

fn tuile_a_trait() -> Tile {
    let mut px = [Color32::TRANSPARENT; TILE_PIXELS];
    for (i, p) in px.iter_mut().enumerate() {
        *p = pixel(i);
    }
    Tile::new(px)
}

/// Les pixels d'un dessin, à plat (la feuille).
fn a_plat(doc: &Document, id: CelId) -> Vec<Color32> {
    doc.cels[&id].raster.to_flat()
}

/// Un document de `n` plans d'un calque bitmap chacun, un trait à des endroits différents ; rend aussi les images de départ.
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
            // (Un nombre de tuiles différent d'un plan à l'autre : les images des plans ne sont pas toutes pareilles.)
            for t in 0..(3 + i % 8) {
                let tile = cel.raster.tile_mut(t);
                for k in 0..TILE_PIXELS {
                    let p = pixel(k);
                    if p != Color32::TRANSPARENT {
                        tile[k] = p;
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

#[test]
fn a_frozen_tile_comes_back_exact_and_a_write_forgets_the_buffer() {
    let mut t = tuile_a_trait();
    let avant: Vec<Color32> = t.to_vec();
    assert!(!t.est_rangee() && t.est_a_ranger());
    assert!(t.geler(), "une tuile seule se range et lâche ses pixels");
    assert!(t.est_rangee() && !t.est_a_ranger());
    assert!(t.octets_en_memoire() < 4096, "{} octets", t.octets_en_memoire());
    let ouvertes = crate::raster::tuiles_rouvertes();
    assert_eq!(t[..], avant[..], "mêmes pixels à la lecture");
    assert!(crate::raster::tuiles_rouvertes() > ouvertes);
    // Une écriture : le tampon est faux, il disparaît ; on range de nouveau, et le nouveau contenu est celui qui revient.
    t[3] = Color32::RED;
    assert!(t.est_a_ranger(), "après une écriture, le tampon est à refaire");
    assert!(!t.est_lachable());
    assert!(t.geler());
    assert_eq!(t[3], Color32::RED);
    assert_eq!(t[4..], avant[4..]);
    // Une tuile rouverte par une lecture garde son tampon : la lâcher de nouveau ne coûte rien.
    assert!(t.est_lachable() || t.est_rangee());
}

#[test]
fn a_shared_tile_lets_each_holder_go_and_the_pixels_leave_with_the_last() {
    let mut a = tuile_a_trait();
    let mut b = a.clone();
    let avant: Vec<Color32> = a.to_vec();
    assert!(a.ranger());
    // Deux dessins (un plan copié) tiennent la même tuile : le premier passe à une tuile à lui, faite du seul tampon ; l'autre garde les pixels.
    assert!(a.lacher_les_pixels());
    assert!(a.est_rangee(), "ce dessin-ci n'a plus les pixels en mémoire");
    assert!(!b.est_rangee(), "l'autre les a encore");
    assert_eq!(b[..], avant[..]);
    assert_eq!(a[..], avant[..], "et les deux voient les mêmes pixels");
    // Seul à tenir l'ancienne tuile, le second la rend sur place.
    assert!(b.lacher_les_pixels());
    assert!(b.est_rangee());
    assert_eq!(b[..], avant[..]);
    // Une copie qu'on écrit ne change pas l'original rangé (copie à l'écriture).
    let mut copie = a.clone();
    copie[0] = Color32::GREEN;
    assert_eq!(a[0], avant[0]);
    assert_eq!(copie[0], Color32::GREEN);
    assert_eq!(copie[1..], avant[1..]);
}

#[test]
fn a_clone_in_flight_keeps_its_pixels_while_the_drawing_lets_go() {
    // Un fil de fond (vignette, export, enregistrement) tient une copie de la tuile pendant que le dessin la range : le fil lit ses pixels, intacts.
    let mut dessin = tuile_a_trait();
    let en_vol = dessin.clone();
    let avant: Vec<Color32> = dessin.to_vec();
    dessin.geler();
    assert!(dessin.est_rangee());
    assert!(!en_vol.est_rangee(), "la copie en vol a toujours ses pixels");
    assert_eq!(en_vol[..], avant[..]);
    drop(en_vol);
    assert_eq!(dessin[..], avant[..]);
}

#[test]
fn a_background_thread_makes_the_buffers_and_the_tiles_then_leave() {
    let mut t = tuile_a_trait();
    let avant: Vec<Color32> = t.to_vec();
    let copie = t.clone();
    let fil = std::thread::spawn(move || {
        copie.ranger();
        copie
    });
    let copie = fil.join().unwrap();
    // Le tampon fait par le fil sert à toutes les copies ; la copie du fil garde ses pixels, la tuile du dessin passe à une tuile à elle.
    assert!(t.est_lachable(), "le tampon fait sur la copie est celui de la tuile");
    assert!(t.lacher_les_pixels());
    assert!(t.est_rangee());
    assert!(!copie.est_rangee(), "la copie du fil lit toujours ses pixels");
    assert_eq!(copie[..], avant[..]);
    drop(copie);
    // Un fil de fond qui rouvre les pixels sur une copie : l'original les voit rouverts.
    let c2 = t.clone();
    std::thread::spawn(move || c2.rouvrir()).join().unwrap();
    assert!(!t.est_rangee());
    assert_eq!(t[..], avant[..]);
}

#[test]
fn the_sweep_ranges_what_is_not_watched_and_keeps_every_pixel() {
    let (mut doc, ids) = doc_de_plans(12);
    let avant: HashMap<CelId, Vec<Color32>> = ids.iter().map(|&id| (id, a_plat(&doc, id))).collect();
    let octets = |doc: &Document| doc.cels.values().map(|c| c.octets_en_memoire()).sum::<usize>();
    let plein = octets(&doc);
    let mut gel = Gel::default();
    gel.activer(true);
    // Plan courant : le premier. Le tour tourne jusqu'à ce que les plans éloignés soient rangés.
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(10) {
        gel.tenir(&mut doc, 0, None);
        std::thread::sleep(Duration::from_millis(2));
        if ids.iter().skip(8).all(|id| !doc.cels[id].a_des_tuiles_ouvertes()) {
            break;
        }
    }
    for (i, id) in ids.iter().enumerate() {
        let ouvert = doc.cels[id].a_des_tuiles_ouvertes();
        if i <= 2 {
            assert!(ouvert, "le plan {i} est chaud : ses pixels restent");
        } else if i >= 8 {
            assert!(!ouvert, "le plan {i} est loin : ses pixels sont rangés");
        }
    }
    assert!(octets(&doc) < plein / 2, "{} -> {} octets", plein, octets(&doc));
    // Tout revient à l'identique, par n'importe quel lecteur.
    for &id in &ids {
        assert_eq!(a_plat(&doc, id), avant[&id], "dessin {id}");
    }
}

#[test]
fn a_drawing_modified_after_being_ranged_is_never_lost() {
    let (mut doc, ids) = doc_de_plans(10);
    let mut gel = Gel::default();
    gel.activer(true);
    let attendre = |gel: &mut Gel, doc: &mut Document, courant: usize| {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(10) {
            gel.tenir(doc, courant, None);
            std::thread::sleep(Duration::from_millis(2));
            if ids.iter().skip(courant + 4).all(|id| !doc.cels[id].a_des_tuiles_ouvertes()) {
                break;
            }
        }
    };
    attendre(&mut gel, &mut doc, 0);
    let loin = ids[9];
    assert!(!doc.cels[&loin].a_des_tuiles_ouvertes(), "le plan 9 est rangé");
    // On y dessine (comme un trait, une propagation, un collage) : la tuile rouverte reçoit le point.
    {
        let cel = doc.cels.get_mut(&loin).unwrap();
        cel.raster.tile_mut(7)[100] = Color32::RED;
        cel.version += 1;
    }
    let mut attendu = a_plat(&doc, loin);
    // Le tour passe plusieurs fois pendant que le dessin reste modifié et jamais regardé : rien ne se perd.
    for _ in 0..3 {
        attendre(&mut gel, &mut doc, 0);
        assert_eq!(a_plat(&doc, loin), attendu);
    }
    let cel = doc.cels.get_mut(&loin).unwrap();
    cel.raster.tile_mut(7)[101] = Color32::BLUE;
    attendu = a_plat(&doc, loin);
    attendre(&mut gel, &mut doc, 0);
    assert_eq!(a_plat(&doc, loin), attendu);
}

#[test]
fn old_undo_steps_are_ranged_and_undo_and_redo_stay_exact() {
    use crate::history::{Edit, History, Nav};
    let (mut doc, ids) = doc_de_plans(2);
    let id = ids[0];
    let mut h = History::default();
    let mut nav = Nav { panel: 0, layer: 0 };
    let mut etats = vec![a_plat(&doc, id)];
    for k in 0..24u8 {
        // Un geste : la tuile 1 est sauvée dans l'étape, puis modifiée (la tuile de l'étape est alors seule à tenir les anciens pixels).
        let cel = doc.cels.get_mut(&id).unwrap();
        let avant = cel.raster.slot(1).cloned();
        h.push(Edit::Tiles { cel: id, tiles: vec![(1, avant)] });
        cel.raster.tile_mut(1)[k as usize] = Color32::from_rgba_premultiplied(k, 200, 3, 255);
        cel.version += 1;
        etats.push(a_plat(&doc, id));
    }
    let (ouvertes, total, octets) = h.tuiles();
    assert_eq!((ouvertes, total), (24, 24), "toutes les étapes sont en pixels au départ");
    let rangees = h.geler_les_vieilles_etapes(8, 1000);
    assert_eq!(rangees, 16, "les 16 plus vieilles étapes se rangent, les 8 dernières restent en pixels");
    let (ouvertes_apres, _, octets_apres) = h.tuiles();
    assert_eq!(ouvertes_apres, 8);
    assert!(octets_apres < octets / 2, "{octets} -> {octets_apres} octets");
    assert_eq!(h.geler_les_vieilles_etapes(8, 1000), 0, "rien de plus à ranger");
    // Tout s'annule, étape après étape, jusqu'au dessin d'origine ; puis tout se rétablit : chaque état est celui d'avant, au pixel près.
    for k in (0..24).rev() {
        assert!(h.undo(&mut doc, &mut nav));
        assert_eq!(a_plat(&doc, id), etats[k], "après avoir annulé jusqu'à l'état {k}");
    }
    for k in 1..=24 {
        assert!(h.redo(&mut doc, &mut nav));
        assert_eq!(a_plat(&doc, id), etats[k], "après avoir rétabli jusqu'à l'état {k}");
    }
}

#[test]
fn undoing_the_removal_of_a_plan_after_its_drawings_were_ranged_gives_the_exact_drawings() {
    use crate::perf_tests::banc;
    let (doc, ids) = doc_de_plans(8);
    let avant: HashMap<CelId, Vec<Color32>> = ids.iter().map(|&id| (id, a_plat(&doc, id))).collect();
    let mut b = banc(doc, false);
    b.app.gel.activer(true);
    let cible = b.app.doc.panels[5].id;
    b.app.select_panel(5);
    b.app.structure_op(|d, _| d.remove_panels(&[cible]));
    assert!(b.app.doc.panels.iter().all(|p| p.id != cible));
    let cel_supprimee = ids[5];
    // Le tour de la tenue : le dessin du plan supprimé n'est plus dans aucun plan, il se range.
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(10) && b.app.doc.cels[&cel_supprimee].a_des_tuiles_ouvertes() {
        b.app.gel.tenir(&mut b.app.doc, 0, None);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!b.app.doc.cels[&cel_supprimee].a_des_tuiles_ouvertes(), "le dessin d'un plan supprimé est rangé");
    // Annuler : le plan revient, ses pixels sont exacts.
    b.app.undo();
    assert!(b.app.doc.panels.iter().any(|p| p.id == cible), "le plan est revenu");
    for &id in &ids {
        assert_eq!(a_plat(&b.app.doc, id), avant[&id], "dessin {id} après annulation");
    }
}

#[test]
fn the_drawings_of_a_project_in_another_tab_are_ranged_too() {
    use crate::perf_tests::banc;
    let (doc, ids) = doc_de_plans(8);
    let avant: HashMap<CelId, Vec<Color32>> = ids.iter().map(|&id| (id, a_plat(&doc, id))).collect();
    let mut b = banc(doc, false);
    assert!(b.app.nouvel_onglet(), "{}", b.app.status);
    b.app.welcome = false;
    let t = Instant::now();
    loop {
        b.chauffer(2);
        let range = b.app.onglets[0].etat.as_ref().expect("rangé");
        // (Le plan courant de l'onglet et ses voisins restent ouverts ; les plans éloignés se rangent.)
        if ids.iter().skip(4).all(|id| !range.doc.cels[id].a_des_tuiles_ouvertes()) {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(10), "les dessins de l'autre onglet ne se rangent pas");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(b.app.activer_onglet(0));
    for &id in &ids {
        assert_eq!(a_plat(&b.app.doc, id), avant[&id], "dessin {id} après être revenu sur l'onglet");
    }
}

#[test]
fn a_project_with_ranged_drawings_saves_the_same_bytes_and_reloads_identical() {
    use crate::enregistrement_cliche_tests::projet_riche;
    use crate::fiabilite_fixture::{arbre, dossier};
    let travail = dossier("gel-enregistrement");
    let mut doc = projet_riche(&travail);
    let utilises: Vec<CelId> = doc.used_cels().into_iter().collect();
    let avant: HashMap<CelId, Vec<Color32>> = utilises.iter().map(|&id| (id, a_plat(&doc, id))).collect();
    let (a, b) = (travail.join("A").join("Film.shot"), travail.join("B").join("Film.shot"));
    crate::project::save(&doc, &a, &mut Default::default()).expect("enregistrement");
    // Tous les dessins se rangent.
    let mut rangees = 0;
    for c in doc.cels.values_mut() {
        rangees += c.geler();
    }
    assert!(rangees > 0, "des tuiles se rangent");
    assert!(doc.cels.values().all(|c| !c.a_des_tuiles_a_ranger()));
    crate::project::save(&doc, &b, &mut Default::default()).expect("enregistrement des dessins rangés");
    assert_eq!(arbre(a.parent().unwrap()), arbre(b.parent().unwrap()), "mêmes fichiers, octet pour octet");
    // Relire : mêmes pixels, et la relecture range elle-même.
    let relu = crate::project::load(&b).expect("relecture");
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let ouverts: Vec<String> = relu.doc.cels.iter().filter(|(_, c)| c.a_des_tuiles_ouvertes()).map(|(id, c)| format!("{id} (vectoriel {}, tuiles {:?}, fond {})", c.vector.is_some(), c.raster.tuiles_ouvertes(), c.fond.is_some())).collect();
    // (Seule la tuile de bord d'un dessin qui déborde de la feuille est ouverte : l'enregistrement l'a lue pour le savoir ; le tour de la tenue la range.)
    let tuiles_ouvertes: usize = relu.doc.cels.values().map(|c| c.raster.tuiles_ouvertes().0).sum();
    assert!(tuiles_ouvertes <= 2, "un projet relu a ses pixels rangés ; ouverts : {ouverts:?}");
    for &id in &utilises {
        assert_eq!(a_plat(&relu.doc, id), avant[&id], "dessin {id} relu");
    }
    // (Et ce que les dessins ont hors de la feuille.)
    for &id in &utilises {
        let (x, y) = (doc.cels[&id].raster.beyond_sheet_tiles(), relu.doc.cels[&id].raster.beyond_sheet_tiles());
        assert_eq!(x.len(), y.len(), "hors-feuille du dessin {id}");
        for ((kx, tx), (ky, ty)) in x.iter().zip(&y) {
            assert!(kx == ky && tx == ty, "hors-feuille du dessin {id}");
        }
    }
    let _ = std::fs::remove_dir_all(&travail);
}

/// L'image finale d'un export (plan `p`, image 0), composée comme le film.
fn image_exportee(doc: &Document, p: usize) -> Vec<Color32> {
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    let snap = snapshot_of(doc, &[p]);
    let rendu = Rendu::new(snap, Papier::Fond, false, Vec::new(), std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
    (*rendu.image(p, 0).expect("image")).clone()
}

#[test]
fn an_export_of_ranged_drawings_is_the_same_image() {
    let (mut doc, ids) = doc_de_plans(6);
    let avant: Vec<Vec<Color32>> = (0..6).map(|p| image_exportee(&doc, p)).collect();
    assert!(avant[0] != avant[1], "les images des plans ne sont pas toutes la même page blanche");
    let mut rangees = 0;
    for &id in &ids {
        rangees += doc.cels.get_mut(&id).unwrap().geler();
    }
    assert!(rangees > 0);
    assert!(ids.iter().all(|id| !doc.cels[id].a_des_tuiles_ouvertes()), "tous les dessins sont rangés avant l'export");
    for p in 0..6 {
        assert_eq!(image_exportee(&doc, p), avant[p], "image exportée du plan {p}");
    }
    // Et la vignette du plan (les fils de fond la composent sur un cliché).
    let cam = doc.panel_camera(2, 0);
    let apres = crate::render::thumbnail(&doc, &crate::render::panel_plan_at(&doc, 2, 0, doc.render_paper()), &cam, 96, 54);
    // (Les mêmes dessins, jamais rangés : un projet neuf de même fabrication.)
    let (neuf, _) = doc_de_plans(6);
    let vignette = crate::render::thumbnail(&neuf, &crate::render::panel_plan_at(&neuf, 2, 0, neuf.render_paper()), &neuf.panel_camera(2, 0), 96, 54);
    assert_eq!(vignette.pixels, apres.pixels, "la vignette d'un plan rangé est la même");
}

#[test]
fn the_recovery_copy_of_a_project_with_ranged_drawings_is_complete() {
    use crate::enregistrement_cliche_tests::projet_riche;
    use crate::fiabilite_fixture::dossier;
    use crate::recovery::{self, Recovery, Snapshot};
    let travail = dossier("gel-secours");
    let mut doc = projet_riche(&travail);
    let utilises: Vec<CelId> = doc.used_cels().into_iter().collect();
    let avant: HashMap<CelId, Vec<Color32>> = utilises.iter().map(|&id| (id, a_plat(&doc, id))).collect();
    for c in doc.cels.values_mut() {
        c.geler();
    }
    let racine = dossier("gel-secours-racine");
    let aucun = HashMap::new();
    let snap = |serial| Snapshot { doc: &doc, serial, modified: true, title: "Film".into(), project: None, project_written: &aucun };
    let mut rec = Recovery::start(&racine).expect("séance");
    rec.start_copy(snap(1));
    let t = Instant::now();
    while !rec.tick(snap(1), None, false) {
        assert!(rec.error.is_none(), "{:?}", rec.error);
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    drop(rec);
    let trouvees = recovery::orphans(&racine, None);
    assert_eq!(trouvees.len(), 1);
    let relu = crate::project::load(&trouvees[0].manifest).expect("la copie se relit");
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    for &id in &utilises {
        assert_eq!(a_plat(&relu.doc, id), avant[&id], "dessin {id} de la copie de secours");
        assert!(!relu.doc.cels[&id].is_empty() || avant[&id].iter().all(|p| *p == Color32::TRANSPARENT));
    }
    let _ = std::fs::remove_dir_all(&racine);
    let _ = std::fs::remove_dir_all(&travail);
}
