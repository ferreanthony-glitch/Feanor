//! Les vieilles étapes d'annulation comprimées (`histocomprime.rs`) : comprimées en fond, décomprimées exactement à
//! l'application, les récentes jamais touchées.

use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::document::{Cel, CelId, Document, LayerKind};
use crate::history::{Edit, History, Nav};
use crate::histocomprime::{comprimer_tuile, decomprimer_tuile, GARDER};
use crate::raster::{empty_tile, Tile, TILE_PIXELS};

fn tuile_motif(k: usize) -> Tile {
    let mut t = empty_tile();
    for (i, px) in t.iter_mut().enumerate() {
        if (i + k * 5) % 7 < 3 {
            *px = Color32::from_rgba_premultiplied((k * 11) as u8, (i % 251) as u8, 40, 200);
        }
    }
    t
}

/// Un dessin et `n` étapes de tuiles (une tuile chacune, deux étapes sur trois) ; rend aussi l'état du dessin avant chaque
/// étape et à la fin.
fn film_d_etapes(n: usize) -> (Document, CelId, History, Vec<Vec<Color32>>) {
    let mut doc = Document::with_frame(160, 90, 24);
    let cel = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
    let mut h = History::default();
    let mut etats = Vec::new();
    for k in 0..n {
        etats.push(doc.cels.get(&cel).expect("présent").raster.to_flat());
        let i = k % 4;
        let c = doc.cels.get_mut(&cel).expect("présent");
        let ancienne = c.raster.replace_slot(i, Some(tuile_motif(k)));
        c.version += 1;
        h.push(Edit::Tiles { cel, tiles: vec![(i, ancienne)] });
    }
    etats.push(doc.cels.get(&cel).expect("présent").raster.to_flat());
    (doc, cel, h, etats)
}

fn attendre_la_compression(h: &mut History, attendues: usize) {
    let t = Instant::now();
    while h.compression.comprimees < attendues && t.elapsed() < Duration::from_secs(20) {
        h.comprimer_les_anciennes(GARDER, None);
        std::thread::sleep(Duration::from_millis(2));
    }
    h.comprimer_les_anciennes(GARDER, None);
}

#[test]
fn a_tile_compresses_and_comes_back_identical() {
    for k in 0..6 {
        let t = tuile_motif(k);
        let z = comprimer_tuile(&t);
        assert!(z.len() < TILE_PIXELS * 4 / 2, "la tuile se comprime ({} octets)", z.len());
        assert!(decomprimer_tuile(&z).expect("décomprimée") == t, "même tuile");
    }
    let vide = empty_tile();
    assert!(decomprimer_tuile(&comprimer_tuile(&vide)).expect("décomprimée") == vide);
    assert!(decomprimer_tuile(&[1, 2, 3]).is_none(), "des octets abîmés ne donnent pas de tuile");
}

#[test]
fn old_steps_compress_in_the_background_and_undo_restores_them_exactly() {
    let n = 60;
    let (mut doc, cel, mut h, etats) = film_d_etapes(n);
    let avant = h.tuiles_gardees();
    // (Les quatre premières étapes remplissent des tuiles vides : rien à garder, rien à comprimer.)
    assert_eq!(avant, n - 4);
    attendre_la_compression(&mut h, n - GARDER - 4);
    assert_eq!(h.compression.comprimees, n - GARDER - 4, "toutes les étapes anciennes qui ont des tuiles sont comprimées");
    assert_eq!(h.tuiles_gardees(), GARDER, "seules les {GARDER} récentes gardent leurs tuiles");
    let z = h.octets_comprimes();
    assert!(z > 0 && z < (n - GARDER - 4) * TILE_PIXELS * 4 / 4, "les étapes anciennes tiennent bien moins que leurs tuiles ({z} octets)");
    // Annuler tout : exactement l'état d'avant chaque étape.
    let mut nav = Nav { panel: 0, layer: 0 };
    for k in (0..n).rev() {
        assert!(h.undo(&mut doc, &mut nav));
        assert!(doc.cels.get(&cel).expect("présent").raster.to_flat() == etats[k], "état d'avant l'étape {k}");
    }
    // Puis tout rétablir.
    for k in 0..n {
        assert!(h.redo(&mut doc, &mut nav));
        assert!(doc.cels.get(&cel).expect("présent").raster.to_flat() == etats[k + 1], "état d'après l'étape {k}");
    }
}

#[test]
fn recent_steps_are_never_compressed_and_groups_compress_by_leaf() {
    let (mut doc, cel, mut h, etats) = film_d_etapes(30);
    // Les 12 dernières en un groupe (comme un geste fait de plusieurs étapes) : plus que GARDER étapes de moins.
    h.merge_last(12);
    attendre_la_compression(&mut h, 7);
    assert_eq!(h.compression.comprimees, 7);
    // Le groupe est parmi les GARDER plus récents : intact ; les étapes d'avant sont comprimées.
    assert!(h.tuiles_gardees() >= 12, "le groupe récent garde ses tuiles");
    let mut nav = Nav { panel: 0, layer: 0 };
    assert!(h.undo(&mut doc, &mut nav));
    assert!(doc.cels.get(&cel).expect("présent").raster.to_flat() == etats[18], "le groupe s'annule d'un coup");
    for k in (0..18).rev() {
        assert!(h.undo(&mut doc, &mut nav));
        assert!(doc.cels.get(&cel).expect("présent").raster.to_flat() == etats[k], "état d'avant l'étape {k}");
    }
}

#[test]
fn a_step_applied_while_it_was_being_compressed_is_not_replaced() {
    let (mut doc, cel, mut h, etats) = film_d_etapes(40);
    let mut nav = Nav { panel: 0, layer: 0 };
    // La compression part, puis on annule tout aussitôt (avant d'avoir regardé les résultats).
    h.comprimer_les_anciennes(GARDER, None);
    for _ in 0..40 {
        assert!(h.undo(&mut doc, &mut nav));
    }
    assert!(doc.cels.get(&cel).expect("présent").raster.to_flat() == etats[0]);
    // Les résultats arrivent pour des étapes qui ne sont plus celles-là : rien n'est remplacé de travers.
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(300) {
        h.comprimer_les_anciennes(GARDER, None);
        std::thread::sleep(Duration::from_millis(5));
    }
    for k in 0..40 {
        assert!(h.redo(&mut doc, &mut nav));
        assert!(doc.cels.get(&cel).expect("présent").raster.to_flat() == etats[k + 1], "rétablir {k}");
    }
}
