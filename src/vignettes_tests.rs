//! Essais des « Vignettes simples » (2026-10-21) : la Planche de crayonnés disparaît (ses essais sont
//! partis avec elle), un ancien `crayonnes.json` reste intact ; la vue Vignettes se dessine
//! directement (voir `board.rs` pour la mise en page et `boardui.rs` pour l'écran).

use crate::document::Document;
use crate::project;

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-vignettes-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

/// Un `crayonnes.json` comme l'écrivait la version d'avant : quatre cases, dont deux dessinées et jamais
/// transformées en plans, une peinte et déjà servie, une avec seulement une légende.
const ANCIEN: &str = r#"{
  "format": "shot-crayonnes",
  "version": 1,
  "grille": [4, 3],
  "taille": [640, 360],
  "cases": [
    { "id": 1, "legende": "Léa sort", "traits": [ { "stroke": "illisible pour cet essai" } ] },
    { "id": 2, "utilisee": true, "peinture": "iVBORw0KGgo=" },
    { "id": 3, "peinture": "iVBORw0KGgo=" },
    { "id": 4, "legende": "Plan large" }
  ]
}"#;

#[test]
fn the_planche_de_crayonnes_has_no_command_left() {
    use crate::keymap::{ACTIONS, DEFAULT_KEYMAP};
    let reste: Vec<&str> = ACTIONS.iter().map(|a| a.id).filter(|id| id.starts_with("crayonnes.") || *id == "storyboard.crayonnes").collect();
    assert!(reste.is_empty(), "commandes de la planche de crayonnés encore là : {reste:?}");
    assert!(!DEFAULT_KEYMAP.contains("action = \"crayonnes.") && !DEFAULT_KEYMAP.contains("storyboard.crayonnes"), "keymap.toml : plus de bloc de crayonnés");
    // (« Crayonné multiple » est un effet du trait, une autre chose : il reste.)
    assert!(ACTIONS.iter().any(|a| a.id == "fx.crayonne"));
}

#[test]
fn counting_the_forgotten_boxes_of_an_old_file() {
    assert_eq!(crate::crayonnesheritage::cases_oubliees(ANCIEN), Some(2), "deux cases dessinées jamais servies");
    assert_eq!(crate::crayonnesheritage::cases_oubliees("pas du json"), None);
    assert_eq!(crate::crayonnesheritage::cases_oubliees("{\"cases\": []}"), Some(0));
    assert_eq!(crate::crayonnesheritage::cases_oubliees(&format!("\u{feff}{ANCIEN}")), Some(2), "avec un BOM");
}

#[test]
fn a_project_with_an_old_crayonnes_file_opens_says_so_and_never_loses_the_file() {
    let dir = dossier("ancien");
    let manifest = dir.join("P.pelure");
    let doc = Document::with_frame(160, 90, 24);
    project::save(&doc, &manifest, &mut Default::default()).expect("enregistrement");
    assert!(!dir.join("crayonnes.json").exists(), "un projet neuf n'écrit aucun crayonnes.json");
    let fichier = dir.join("crayonnes.json");
    std::fs::write(&fichier, ANCIEN).unwrap();

    let loaded = project::load(&manifest).expect("le projet s'ouvre");
    let avis: Vec<&String> = loaded.warnings.iter().filter(|w| w.contains("crayonnes.json")).collect();
    assert_eq!(avis.len(), 1, "un seul avertissement : {:?}", loaded.warnings);
    assert!(avis[0].contains("2 cases dessinées") && avis[0].contains("Vignettes"), "{}", avis[0]);
    assert_eq!(std::fs::read_to_string(&fichier).unwrap(), ANCIEN, "ouvrir ne touche pas au fichier");

    // Enregistrer (plusieurs fois, comme un travail de plusieurs jours) ne l'efface pas non plus.
    let mut written = loaded.written.clone();
    project::save(&loaded.doc, &manifest, &mut written).expect("enregistrement");
    project::save(&loaded.doc, &manifest, &mut written).expect("enregistrement");
    assert_eq!(std::fs::read_to_string(&fichier).unwrap(), ANCIEN, "enregistrer ne touche pas au fichier");
    // Et le projet rouvert est le même : la planche n'est nulle part dans le document.
    let again = project::load(&manifest).expect("réouverture");
    assert_eq!(again.doc.panels.len(), doc.panels.len());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unreadable_or_empty_crayonnes_file_is_silent_and_untouched() {
    for (nom, contenu) in [("abime", "{ pas du json"), ("vide", "{\"format\":\"shot-crayonnes\",\"version\":1,\"grille\":[4,3],\"taille\":[640,360],\"cases\":[{\"id\":1,\"legende\":\"Seulement une légende\"}]}")] {
        let dir = dossier(nom);
        let manifest = dir.join("P.pelure");
        project::save(&Document::with_frame(160, 90, 24), &manifest, &mut Default::default()).unwrap();
        let fichier = dir.join("crayonnes.json");
        std::fs::write(&fichier, contenu).unwrap();
        let loaded = project::load(&manifest).expect("le projet s'ouvre quand même");
        assert!(loaded.warnings.iter().all(|w| !w.contains("crayonnes.json")), "{nom} : pas d'avertissement ({:?})", loaded.warnings);
        project::save(&loaded.doc, &manifest, &mut loaded.written.clone()).unwrap();
        assert_eq!(std::fs::read_to_string(&fichier).unwrap(), contenu, "{nom} : le fichier reste tel quel");
        std::fs::remove_dir_all(&dir).ok();
    }
}
