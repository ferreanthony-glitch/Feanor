//! Le cliché léger de l'enregistrement (`project::prepare_save`) : le fil de l'interface ne copie que le document sans
//! ses dessins et le contenu des dessins modifiés ; le manifeste, les annexes et la lecture du dossier se font sur le fil
//! d'écriture. Mêmes fichiers, octet pour octet, que la préparation d'avant ; un dessin effacé à côté de SHOT est
//! réécrit ; le temps du fil de l'interface reste petit sur un grand film.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::app::Pending;
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::fiabilite_fixture::{arbre, dossier, projet_complet};
use crate::perf_tests::{avec_notes_et_cameras, banc, Banc};
use crate::project;
use crate::raster::{Tile, TILE_PIXELS};
use crate::recovery::{Recovery, Snapshot};

/// Le projet complet de la fiabilité, plus ce qu'il n'a pas : une version de plan, une variante rangée, un dessin qui a
/// quelque chose hors de la feuille.
pub(crate) fn projet_riche(travail: &Path) -> Document {
    let mut doc = projet_complet(travail).doc;
    doc.nouvelle_version(1, 1_760_000_000);
    let seq = doc.sequence_of(0);
    doc.nouvelle_variante(seq, false, 1_760_000_100);
    let id = doc.panels.iter().flat_map(|p| &p.layers).flat_map(|l| l.frames.iter().flatten()).copied().find(|id| doc.cels[id].vector.is_none()).expect("un dessin bitmap");
    let cel = doc.cels.get_mut(&id).expect("dessin");
    cel.raster.absorb_beyond(-2, -1, Tile::new([Color32::from_rgba_premultiplied(40, 90, 200, 255); TILE_PIXELS]));
    cel.version += 1;
    doc
}

/// Les fichiers d'un dossier de projet, sans le nom du dossier.
fn fichiers(manifeste: &Path) -> std::collections::BTreeMap<String, u64> {
    arbre(manifeste.parent().expect("dossier du projet"))
}

/// Les fichiers qui diffèrent entre deux dossiers de projet (absents d'un côté compris).
fn differences(a: &Path, b: &Path) -> Vec<String> {
    let (fa, fb) = (fichiers(a), fichiers(b));
    fa.keys().chain(fb.keys()).filter(|k| fa.get(*k) != fb.get(*k)).cloned().collect::<std::collections::BTreeSet<_>>().into_iter().collect()
}

#[test]
fn the_light_snapshot_writes_exactly_what_the_old_preparation_wrote() {
    let travail = dossier("cliche-memes-octets");
    let doc = projet_riche(&travail);
    let (avant, apres) = (travail.join("Avant").join("Film.pelure"), travail.join("Apres").join("Film.pelure"));
    let (mut w_avant, mut w_apres) = (HashMap::new(), HashMap::new());
    for passe in ["premier enregistrement", "rien de modifié"] {
        project::prepare_save_avant(&doc, &avant, &mut w_avant).expect("préparation d'avant").run().expect("écriture d'avant");
        let rapport = project::prepare_save(&doc, &apres, &mut w_apres, None).expect("cliché").run().expect("écriture");
        assert!(rapport.manquants.is_empty(), "{passe} : {:?}", rapport.manquants);
        assert_eq!(w_avant, w_apres, "{passe} : mêmes dessins retenus comme écrits");
        assert_eq!(differences(&avant, &apres), Vec::<String>::new(), "{passe} : mêmes fichiers, octet pour octet");
    }
    // Ce que le dossier garde vraiment : le hors-feuille, la version, la variante, et le manifeste qui les cite.
    let noms: Vec<String> = fichiers(&apres).into_keys().collect();
    for attendu in ["_hors.png", "_hors.json", crate::planvers::FILE, crate::variantes::FILE, crate::commentaires::FILE, crate::references::FILE] {
        assert!(noms.iter().any(|n| n.ends_with(attendu)), "{attendu} manque : {noms:?}");
    }
    // Le manifeste préparé est celui du document vivant.
    let mut job = project::prepare_save(&doc, &apres, &mut w_apres, None).expect("cliché");
    assert_eq!(project::manifeste_du_travail(&mut job), project::manifeste_vivant(&doc));
    let _ = std::fs::remove_dir_all(&travail);
}

#[test]
fn the_snapshot_is_taken_at_the_call_not_when_the_writer_runs() {
    let travail = dossier("cliche-instant");
    let mut doc = projet_riche(&travail);
    let manifeste = travail.join("Film").join("Film.pelure");
    let mut written = HashMap::new();
    let attendu = project::manifeste_vivant(&doc);
    let commentaires = crate::commentaires::to_bytes(&doc).expect("commentaires").expect("le projet en a");
    // Le même projet, écrit d'un coup avant tout changement : ce que le cliché doit donner.
    let reference = travail.join("Reference").join("Film.pelure");
    project::prepare_save_avant(&doc, &reference, &mut HashMap::new()).expect("référence").run().expect("référence écrite");
    let job = project::prepare_save(&doc, &manifeste, &mut written, None).expect("cliché");
    // Tout change après le cliché, avant que le fil d'écriture ne commence : rien de cela n'est écrit.
    doc.panels[0].notes = "Écrit après le cliché.".into();
    let plan = doc.panels[0].id;
    doc.add_comment(plan, 2, "Anthony", 9);
    let id = doc.panels.iter().flat_map(|p| &p.layers).flat_map(|l| l.frames.iter().flatten()).copied().find(|id| doc.cels[id].vector.is_none()).expect("un dessin bitmap");
    let cel = doc.cels.get_mut(&id).expect("dessin");
    cel.raster = crate::raster::Raster::new(cel.raster.width, cel.raster.height);
    cel.version += 1;
    job.run().expect("écriture");
    assert_eq!(std::fs::read(&manifeste).expect("manifeste"), attendu, "le manifeste est celui du cliché");
    let dir = manifeste.parent().expect("dossier");
    assert_eq!(std::fs::read(dir.join(crate::commentaires::FILE)).expect("commentaires"), commentaires, "le commentaire posé après le cliché n'est pas écrit");
    assert_eq!(differences(&manifeste, &reference), Vec::<String>::new(), "tout le dossier est celui du cliché, dessin modifié après compris");
    let _ = std::fs::remove_dir_all(&travail);
}

/// Un petit film : `n` dessins bitmap sur un calque.
fn petit_film(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    for k in 0..n {
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(k)[0] = Color32::from_rgba_premultiplied(200, 10, 10, 255);
        layer.frames.push(Some(doc.add_cel(cel)));
    }
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = n;
    doc
}

fn chemin_du_dessin(manifeste: &Path, id: CelId) -> PathBuf {
    manifeste.parent().expect("dossier").join("dessins").join(format!("{id}.png"))
}

#[test]
fn a_drawing_deleted_beside_shot_is_written_again_by_the_next_save() {
    let travail = dossier("cliche-efface");
    let doc = petit_film(4);
    let manifeste = travail.join("Film").join("Film.pelure");
    let mut written = HashMap::new();
    project::save(&doc, &manifeste, &mut written).expect("premier");
    let id = doc.panels[0].layers[0].frames[2].expect("dessin");
    let fichier = chemin_du_dessin(&manifeste, id);
    let octets = std::fs::read(&fichier).expect("dessin écrit");
    std::fs::remove_file(&fichier).expect("effacé à côté de SHOT");
    // Le cliché le croyait écrit : le fil d'écriture voit qu'il manque, et le dit.
    let rapport = project::prepare_save(&doc, &manifeste, &mut written.clone(), None).expect("cliché").run().expect("écriture");
    assert_eq!(rapport.manquants, vec![id]);
    // `project::save` le réécrit dans la foulée, comme l'enregistrement d'avant.
    let rapport = project::save(&doc, &manifeste, &mut written).expect("enregistrement");
    assert!(rapport.manquants.is_empty());
    assert_eq!(std::fs::read(&fichier).expect("réécrit"), octets);
    assert!(project::load(&manifeste).expect("relu").warnings.is_empty());
    let _ = std::fs::remove_dir_all(&travail);
}

fn pomper(b: &mut Banc) {
    for _ in 0..600 {
        if !b.app.save_en_cours() {
            return;
        }
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("l'enregistrement ne se termine pas");
}

#[test]
fn ctrl_s_writes_again_a_drawing_deleted_beside_shot_before_what_follows() {
    let travail = dossier("cliche-efface-interface");
    let manifeste = travail.join("Film").join("Film.pelure");
    let mut b = banc(petit_film(4), false);
    b.app.path = Some(manifeste.clone());
    b.app.modified = true;
    assert!(b.app.save());
    pomper(&mut b);
    let id = b.app.doc.panels[0].layers[0].frames[1].expect("dessin");
    let fichier = chemin_du_dessin(&manifeste, id);
    std::fs::remove_file(&fichier).expect("effacé à côté de SHOT");
    // Ctrl+S puis Quitter : le dessin revient, puis Quitter a lieu.
    assert!(b.app.save());
    b.app.request(Pending::Quit);
    pomper(&mut b);
    assert!(fichier.exists(), "le dessin effacé est réécrit");
    assert!(!b.app.modified && b.app.save_error.is_none(), "{}", b.app.status);
    assert!(b.app.allow_close, "la suite a lieu après le second passage");
    assert!(project::load(&manifeste).expect("relu").warnings.is_empty());
    let _ = std::fs::remove_dir_all(&travail);
}

#[test]
fn the_recovery_copy_writes_again_a_drawing_missing_from_it() {
    let root = dossier("cliche-secours");
    let doc = petit_film(3);
    let aucun = HashMap::new();
    let snap = |serial| Snapshot { doc: &doc, serial, modified: true, title: "Film".into(), project: None, project_written: &aucun };
    let mut rec = Recovery::start(&root).expect("séance");
    let copier = |rec: &mut Recovery, serial: u64| {
        rec.start_copy(snap(serial));
        let t = Instant::now();
        while !rec.tick(snap(serial), None, false) {
            assert!(rec.error.is_none(), "{:?}", rec.error);
            std::thread::sleep(Duration::from_millis(5));
            assert!(t.elapsed() < Duration::from_secs(20));
        }
    };
    copier(&mut rec, 1);
    let manifeste = rec.dir.join("projet").join("secours.pelure");
    let id = doc.panels[0].layers[0].frames[0].expect("dessin");
    let fichier = chemin_du_dessin(&manifeste, id);
    std::fs::remove_file(&fichier).expect("effacé");
    copier(&mut rec, 2);
    copier(&mut rec, 2);
    assert!(fichier.exists(), "la copie suivante réécrit le dessin qui manquait");
    assert!(project::load(&manifeste).expect("copie relue").warnings.is_empty());
    rec.close();
    let _ = std::fs::remove_dir_all(&root);
}

/// Un film de `plans` plans comme celui du banc (`perf_tests::projet_long` : trois calques, 24 images, seize dessins par
/// plan, des notes et deux clés de caméra à chaque plan), sur une petite feuille (les pixels ne comptent pas ici).
fn grand_film(plans: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    let mut at = 0;
    for i in 1..plans {
        at = if i % 10 == 0 { doc.add_sequence_after(at) } else { doc.add_scene_after(at) };
    }
    for p in 0..doc.panels.len() {
        let mut ls = Vec::new();
        for l in 0..3 {
            let every = if l == 2 { 24 } else { 2 + 6 * l };
            let mut layer = Layer::new(&format!("Calque {}", l + 1), LayerKind::Bitmap);
            let mut last = None;
            for f in 0..24 {
                if f % every == 0 {
                    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
                    let n = cel.raster.tiles.len();
                    cel.raster.tile_mut((p * 7 + l * 3 + f) % n)[0] = Color32::from_rgba_premultiplied(9, 9, 9, 255);
                    last = Some(doc.add_cel(cel));
                }
                layer.frames.push(last);
            }
            ls.push(layer);
        }
        doc.panels[p].layers = ls;
        doc.panels[p].length = 24;
    }
    avec_notes_et_cameras(&mut doc, true);
    doc
}

/// Ctrl+S et la copie de secours, rien de modifié : la part du fil de l'interface, avant (tout le travail sur lui) et
/// après (le cliché léger), dans le même processus. La machine des essais est partagée : la borne est large, le
/// rapport avant / après ne l'est pas.
#[test]
fn the_interface_share_of_a_save_stays_small_on_a_big_film() {
    for (plans, borne_ms) in [(100usize, 6.0f64), (600, 15.0)] {
        let doc = grand_film(plans);
        doc.memoriser_dessins();
        let travail = dossier(&format!("cliche-temps-{plans}"));
        let manifeste = travail.join("Film").join("Film.pelure");
        // Tout est déjà écrit (le dossier est vide : le fil de l'interface n'y regarde plus).
        let tous: HashMap<CelId, u64> = doc.used_cels().into_iter().map(|id| (id, doc.cels[&id].version)).collect();
        // Avant et après en alternance, le meilleur de sept. (Le travail préparé part sur le fil d'écriture : il n'est pas
        // jeté dans la mesure, l'application ne le jette pas sur le fil de l'interface.)
        let (mut avant, mut apres, mut gardes) = (f64::MAX, f64::MAX, Vec::new());
        for _ in 0..7 {
            let mut w = tous.clone();
            let t = Instant::now();
            gardes.push(project::prepare_save_avant(&doc, &manifeste, &mut w).expect("avant"));
            avant = avant.min(t.elapsed().as_secs_f64() * 1e3);
            let mut w = tous.clone();
            let t = Instant::now();
            gardes.push(project::prepare_save(&doc, &manifeste, &mut w, None).expect("cliché"));
            apres = apres.min(t.elapsed().as_secs_f64() * 1e3);
        }
        drop(gardes);
        println!("PERF fil de l'interface, enregistrement sans modification, {plans} plans ({} dessins) : avant {avant:.2} ms, après {apres:.2} ms", doc.cels.len());
        assert!(apres < borne_ms, "{plans} plans : {apres:.2} ms sur le fil de l'interface (borne {borne_ms} ms)");
        assert!(apres * 3.0 < avant, "{plans} plans : après {apres:.2} ms, avant {avant:.2} ms");
        let _ = std::fs::remove_dir_all(&travail);
    }
}

/// Changer la résolution en mettant les dessins à l'échelle (Fichier > Réglages du projet), enregistrer, rouvrir : les
/// dessins sont à la nouvelle taille, avec leurs pixels. (Les dessins refaits gardaient la version 0 d'un projet relu :
/// l'enregistrement les croyait déjà écrits et laissait les anciens PNG, refusés à la réouverture.)
#[test]
fn a_resolution_change_with_scaling_is_saved_and_reopened_at_the_new_size() {
    let travail = dossier("cliche-resolution");
    let manifeste = travail.join("Film").join("Film.pelure");
    // (Feuilles en tuiles entières et dessins loin des bords : rien hors de la feuille après la mise à l'échelle.)
    let mut film = Document::with_frame(256, 128, 24);
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    for k in 0..3usize {
        let mut cel = Cel::new(film.width, film.height, LayerKind::Bitmap);
        for y in 100..140 {
            for x in 150 + 60 * k..190 + 60 * k {
                let i = cel.raster.tile_index_of(x, y);
                cel.raster.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = Color32::from_rgba_premultiplied(200, 30, 10, 255);
            }
        }
        layer.frames.push(Some(film.add_cel(cel)));
    }
    film.panels[0].layers = vec![layer];
    film.panels[0].length = 3;
    project::save(&film, &manifeste, &mut HashMap::new()).expect("premier");
    let project::Loaded { mut doc, mut written, .. } = project::load(&manifeste).expect("relu");
    let (w0, h0) = (doc.width, doc.height);
    doc.resize_frame(doc.frame[0] * 2, doc.frame[1] * 2, true);
    assert_eq!((doc.width, doc.height), (w0 * 2, h0 * 2));
    project::save(&doc, &manifeste, &mut written).expect("après le changement de résolution");
    let relu = project::load(&manifeste).expect("rouvert");
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    assert_eq!((relu.doc.width, relu.doc.height), (doc.width, doc.height));
    for (id, cel) in &doc.cels {
        let lu = &relu.doc.cels.get(id).expect("dessin relu").raster;
        assert_eq!((lu.width, lu.height), (cel.raster.width, cel.raster.height), "dessin {id} : nouvelle taille");
        let (a, b) = (cel.raster.to_rgba(), lu.to_rgba());
        let ecart = a.iter().zip(&b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
        assert!(ecart <= 2, "dessin {id} : les pixels mis à l'échelle (écart {ecart})");
        assert!(a.chunks(4).any(|p| p[3] > 0), "dessin {id} : il reste quelque chose à comparer");
    }
    let _ = std::fs::remove_dir_all(&travail);
}