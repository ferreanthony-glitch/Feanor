//! Rendre la mémoire d'un gros projet (`lacher.rs`) : le tas segmenté du manifeste (`build.rs`), le projet remplacé ou
//! l'onglet fermé rendus sur un autre fil, la sortie de SHOT qui ne rend rien.

use std::time::Duration;

use crate::document::{Document, LayerKind};
use crate::history::Edit;
use crate::raster::{empty_tile, Tile, TILE};

/// Un projet dont un dessin tient la tuile rendue (et une étape d'annulation aussi).
fn projet_avec_tuile() -> (Document, Tile, u64) {
    let mut doc = Document::new(2 * TILE, TILE, 24);
    let id = doc.new_cel(LayerKind::Bitmap);
    let tuile = empty_tile();
    doc.cels.get_mut(&id).unwrap().raster.tiles[0] = Some(tuile.clone());
    doc.panels[0].layers[0].frames = vec![Some(id)];
    (doc, tuile, id)
}

#[test]
fn le_tas_du_processus_est_le_tas_segmente() {
    // (Compilé avec `SHOT_TAS_STANDARD` : le manifeste est retiré exprès, pour les mesures de comparaison.)
    if option_env!("SHOT_TAS_STANDARD").is_some() {
        assert_eq!(crate::lacher::tas_segmente(), Some(false));
        return;
    }
    assert_eq!(crate::lacher::tas_segmente(), Some(true), "le manifeste de build.rs demande le tas segmenté : 160 000 tuiles se rendent en 0,7 s au lieu de 8 min");
}

#[test]
fn un_projet_remplace_est_rendu_sur_un_autre_fil() {
    let (doc, tuile, id) = projet_avec_tuile();
    let mut b = crate::perf_tests::banc(doc, false);
    b.app.history.push(Edit::Tiles { cel: id, tiles: vec![(0, Some(tuile.clone()))] });
    let serial = b.app.history.serial();
    assert!(tuile.partages() >= 3, "le dessin et l'étape tiennent la tuile");
    b.app.load_doc(Document::new(2 * TILE, TILE, 24));
    // L'historique est vide tout de suite, et son numéro avance (les gestes en cours et la copie de secours le voient).
    assert!(b.app.history.is_empty());
    assert!(b.app.history.serial() > serial);
    b.chauffer(3);
    assert!(crate::lacher::attendre(Duration::from_secs(20)), "le fil qui lâche l'ancien projet finit");
    assert_eq!(tuile.partages(), 1, "l'ancien projet et son historique sont rendus");
}

#[test]
fn un_onglet_ferme_est_rendu_sur_un_autre_fil() {
    let (doc, tuile, id) = projet_avec_tuile();
    let mut b = crate::perf_tests::banc(doc, false);
    b.app.history.push(Edit::Tiles { cel: id, tiles: vec![(0, Some(tuile.clone()))] });
    assert!(b.app.nouvel_onglet());
    b.chauffer(2);
    assert_eq!(b.app.onglets.len(), 2);
    // L'onglet du projet n'a rien à perdre (rien de modifié) : il se ferme sans passer à l'écran.
    b.app.onglets[0].etat.as_mut().unwrap().modified = false;
    b.app.fermer_onglet(0);
    assert_eq!(b.app.onglets.len(), 1);
    b.chauffer(2);
    assert!(crate::lacher::attendre(Duration::from_secs(20)));
    assert_eq!(tuile.partages(), 1, "le document et l'historique de l'onglet fermé sont rendus");
}

#[test]
fn la_sortie_ne_rend_pas_les_projets() {
    let (doc, tuile, id) = projet_avec_tuile();
    let mut b = crate::perf_tests::banc(doc, false);
    b.app.history.push(Edit::Tiles { cel: id, tiles: vec![(0, Some(tuile.clone()))] });
    b.app.sortie_sans_rendre();
    // Le projet n'est plus dans l'application (ce qui s'y lâche ensuite est léger)…
    assert!(!b.app.doc.cels.contains_key(&id));
    assert!(b.app.history.is_empty());
    // … mais ses pixels ne sont pas rendus un à un : le système reprend la mémoire du processus à sa fin.
    drop(b);
    assert!(crate::lacher::attendre(Duration::from_secs(20)));
    // (L'essai, le dessin et l'étape d'annulation : les caches de l'affichage, eux, sont partis avec l'application.)
    assert_eq!(tuile.partages(), 3, "rien n'est rendu à la sortie");
}
