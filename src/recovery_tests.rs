//! Tests de la copie de secours et de la récupération (phase 4a).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::Color32;

use crate::document::{Document, Layer, LayerKind};
use crate::project;
use crate::recovery::{self, Recovery, Snapshot};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pelure-test-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un panneau, un calque bitmap avec un point rouge.
fn doc() -> Document {
    let mut doc = Document::new(64, 48, 24);
    doc.panels[0].layers = vec![Layer::new("B", LayerKind::Bitmap)];
    let c = doc.new_cel(LayerKind::Bitmap);
    doc.cels.get_mut(&c).unwrap().raster.tile_mut(0)[5] = Color32::RED;
    doc.panels[0].layers[0].frames = vec![Some(c)];
    doc
}

fn snap<'a>(doc: &'a Document, serial: u64, written: &'a HashMap<u64, u64>, project: Option<PathBuf>) -> Snapshot<'a> {
    Snapshot {
        doc,
        serial,
        modified: true,
        title: "Mon film".into(),
        project,
        project_written: written,
    }
}

/// Fait tourner la copie jusqu'au bout.
fn copy_now(rec: &mut Recovery, doc: &Document, serial: u64, written: &HashMap<u64, u64>, project: Option<PathBuf>) {
    rec.start_copy(snap(doc, serial, written, project));
    let t = std::time::Instant::now();
    while !rec.tick(snap(doc, serial, written, None), None, false) {
        assert!(rec.error.is_none(), "{:?}", rec.error);
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(20));
    }
}

#[test]
fn a_live_session_is_not_an_orphan_and_a_crashed_one_is() {
    let root = temp("secours");
    let doc = doc();
    let none = HashMap::new();
    let mut rec = Recovery::start(&root).unwrap();
    assert!(recovery::orphans(&root, None).is_empty(), "séance vivante : verrou tenu");
    copy_now(&mut rec, &doc, 1, &none, None);
    assert!(rec.info.copied.is_some());
    // Plantage : le verrou est relâché sans que le dossier soit effacé.
    let dir = rec.dir.clone();
    drop(rec);
    let found = recovery::orphans(&root, None);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].dir, dir);
    assert_eq!(found[0].info.title, "Mon film");
    // La copie se relit : même dessin.
    let back = project::load(&found[0].manifest).unwrap().doc;
    let cel = back.panels[0].layers[0].frames[0].unwrap();
    assert_eq!(back.cels[&cel].raster.get(5, 0), Color32::RED);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn the_recovery_copy_keeps_the_timeline_comments() {
    let root = temp("secours-commentaires");
    let mut doc = doc();
    let plan = doc.panels[0].id;
    let id = doc.add_comment(plan, 0, "Anthony", 1_760_000_000);
    doc.comment_mut(id).unwrap().text = "À reprendre.".into();
    let none = HashMap::new();
    let mut rec = Recovery::start(&root).unwrap();
    copy_now(&mut rec, &doc, 1, &none, None);
    drop(rec);
    let found = recovery::orphans(&root, None);
    assert_eq!(found.len(), 1);
    assert!(found[0].manifest.parent().unwrap().join(crate::commentaires::FILE).exists(), "commentaires.json suit la copie");
    let back = project::load(&found[0].manifest).unwrap().doc;
    assert_eq!(back.comments, doc.comments);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn sessions_without_a_copy_are_cleaned_and_closing_removes_the_folder() {
    let root = temp("secours-vide");
    let rec = Recovery::start(&root).unwrap();
    let dir = rec.dir.clone();
    drop(rec); // plantage avant toute copie
    assert!(recovery::orphans(&root, None).is_empty());
    assert!(!dir.exists(), "rien à récupérer : effacé");
    let rec = Recovery::start(&root).unwrap();
    let dir = rec.dir.clone();
    rec.close();
    assert!(!dir.exists(), "fermeture normale : effacé");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn copies_follow_the_delay_and_changes() {
    let root = temp("secours-delai");
    let doc = doc();
    let none = HashMap::new();
    let mut rec = Recovery::start(&root).unwrap();
    let wait = |rec: &mut Recovery| {
        let t = std::time::Instant::now();
        while rec.busy() {
            rec.tick(snap(&doc, 0, &none, None), None, false);
            std::thread::sleep(Duration::from_millis(5));
            assert!(t.elapsed() < Duration::from_secs(20));
        }
    };
    // Délai pas encore passé : rien.
    rec.tick(snap(&doc, 1, &none, None), Some(Duration::from_secs(3600)), false);
    assert!(!rec.busy());
    // Délai nul, mais un geste en cours : rien.
    rec.tick(snap(&doc, 1, &none, None), Some(Duration::ZERO), true);
    assert!(!rec.busy());
    // Délai nul : copie.
    rec.tick(snap(&doc, 1, &none, None), Some(Duration::ZERO), false);
    assert!(rec.busy());
    wait(&mut rec);
    // Pas de changement depuis : pas de nouvelle copie ; document enregistré : non plus.
    rec.tick(snap(&doc, 1, &none, None), Some(Duration::ZERO), false);
    assert!(!rec.busy());
    let mut saved = snap(&doc, 2, &none, None);
    saved.modified = false;
    rec.tick(saved, Some(Duration::ZERO), false);
    assert!(!rec.busy());
    rec.tick(snap(&doc, 2, &none, None), Some(Duration::ZERO), false);
    assert!(rec.busy(), "nouvelle modification : nouvelle copie");
    wait(&mut rec);
    rec.close();
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn unchanged_drawings_are_copied_from_the_project() {
    let root = temp("secours-reprise");
    let proj = temp("secours-projet");
    let doc = doc();
    let manifest = proj.join("Film.pelure");
    let mut written = HashMap::new();
    project::save(&doc, &manifest, &mut written).unwrap();
    // On marque le fichier du projet : s'il est recopié (et non réencodé), la marque passe.
    let cel = doc.panels[0].layers[0].frames[0].unwrap();
    let file = |dir: &Path| dir.join("dessins").join(format!("{cel}.png"));
    let mut bytes = std::fs::read(file(&proj)).unwrap();
    bytes.extend_from_slice(b"MARQUE");
    std::fs::write(file(&proj), &bytes).unwrap();
    let mut rec = Recovery::start(&root).unwrap();
    copy_now(&mut rec, &doc, 1, &written, Some(manifest.clone()));
    let copied = std::fs::read(file(&rec.dir.join("projet"))).unwrap();
    assert!(copied.ends_with(b"MARQUE"), "dessin recopié depuis le projet");
    assert_eq!(rec.info.project.as_deref(), Some(manifest.as_path()));
    rec.close();
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&proj).ok();
}

#[test]
fn time_ago() {
    let now = recovery::now_secs();
    assert_eq!(crate::recoveryui::ago(now), "à l'instant");
    assert_eq!(crate::recoveryui::ago(now - 180), "il y a 3 min");
    assert_eq!(crate::recoveryui::ago(now - 7200), "il y a 2 h");
    assert_eq!(crate::recoveryui::ago(now - 86_400), "il y a 1 jour");
}

#[test]
fn autosave_setting_defaults_to_one_minute() {
    let s: crate::settings::Settings = toml::from_str("").unwrap();
    assert_eq!(s.autosave_minutes, 1);
    let s: crate::settings::Settings = toml::from_str("copie_secours_minutes = 0").unwrap();
    assert_eq!(s.autosave_minutes, 0);
}

/// Un film dont la copie de secours prend un peu de temps : 48 dessins pleine feuille de bruit (des PNG lents à coder).
fn film_lent() -> Document {
    let mut doc = Document::with_frame(640, 360, 24);
    let mut bruit = crate::raster::Raster::new(doc.width, doc.height);
    let mut graine = 12_345u32;
    for i in 0..bruit.tiles.len() {
        let tuile = bruit.tile_mut(i);
        for p in tuile.iter_mut() {
            graine = graine.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            *p = Color32::from_rgba_premultiplied((graine >> 8) as u8, (graine >> 16) as u8, (graine >> 24) as u8, 255);
        }
    }
    let mut layer = Layer::new("B", LayerKind::Bitmap);
    for _ in 0..48 {
        let c = doc.new_cel(LayerKind::Bitmap);
        doc.cels.get_mut(&c).unwrap().raster = bruit.clone();
        layer.frames.push(Some(c));
    }
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = 48;
    doc
}

/// La carte graphique perdue pendant qu'une copie de secours s'écrit (`App::secours_carte_perdue` →
/// `Recovery::start_copy`) : la copie demandée part dès la fin de celle-ci, même copie automatique éteinte, et elle a
/// les dernières retouches.
#[test]
fn an_urgent_copy_asked_while_one_is_running_follows_it_at_once() {
    let root = temp("secours-urgent");
    let mut doc = film_lent();
    let none = HashMap::new();
    let mut rec = Recovery::start(&root).unwrap();
    rec.start_copy(snap(&doc, 1, &none, None));
    // Une retouche, puis la carte perdue pendant que la première copie s'écrit.
    doc.panels[0].notes = "Retouche d'après la première copie".into();
    rec.start_copy(snap(&doc, 2, &none, None));
    assert!(rec.busy(), "la première copie s'écrit encore (sinon l'essai ne dit rien)");
    assert!(rec.en_attente(), "la demande est retenue");
    // Copie automatique éteinte (aucun délai) : seule la demande peut lancer la suivante.
    let t = std::time::Instant::now();
    while !rec.tick(snap(&doc, 2, &none, None), None, false) {
        assert!(rec.error.is_none(), "{:?}", rec.error);
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    assert!(rec.busy(), "la copie demandée part à la fin de la première, dans le même passage");
    assert!(!rec.en_attente());
    while !rec.tick(snap(&doc, 2, &none, None), None, false) {
        assert!(rec.error.is_none(), "{:?}", rec.error);
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    let manifest = rec.dir.join("projet").join("secours.pelure");
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("Retouche d'après la première copie"), "la copie a la dernière retouche");
    // Rien de plus ne part ensuite : pas de copie en boucle.
    assert!(!rec.tick(snap(&doc, 2, &none, None), None, false) && !rec.busy());
    rec.close();
    std::fs::remove_dir_all(&root).ok();
}

/// La demande est oubliée si rien n'a changé depuis le cliché de la copie qui s'écrivait, ou si le projet a été enregistré.
#[test]
fn an_urgent_copy_with_nothing_new_or_after_a_save_does_not_start() {
    let root = temp("secours-urgent-rien");
    let doc = film_lent();
    let none = HashMap::new();
    let mut rec = Recovery::start(&root).unwrap();
    rec.start_copy(snap(&doc, 1, &none, None));
    rec.start_copy(snap(&doc, 1, &none, None));
    assert!(rec.busy() && rec.en_attente());
    let t = std::time::Instant::now();
    while !rec.tick(snap(&doc, 1, &none, None), None, false) {
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    assert!(!rec.busy() && !rec.en_attente(), "la copie qui vient de finir avait déjà tout");
    // Enregistré entre-temps (plus de modification) : la demande tombe.
    rec.demander();
    let mut s = snap(&doc, 2, &none, None);
    s.modified = false;
    rec.tick(s, None, false);
    assert!(!rec.busy() && !rec.en_attente());
    rec.close();
    std::fs::remove_dir_all(&root).ok();
}