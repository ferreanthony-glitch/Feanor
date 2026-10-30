//! Les anciens fichiers restent lisibles après le retrait, le 2026-10-30, de l'import et de l'export d'un logiciel de
//! storyboard tiers (et de son jeu de raccourcis) :
//! - un fichier de raccourcis personnel qui cite une action supprimée ne plante ni ne bloque rien : l'action inconnue
//!   est ignorée, avec une remarque ;
//! - une copie de l'ancien `keymap.toml` (avec ses champs `source` et `a_verifier`) n'est toujours pas personnelle ;
//! - un ancien `reglages.toml` (jeu de raccourcis, modèle et dossier d'export du logiciel tiers) se lit sans erreur ;
//! - les projets (le format ne change pas : `project::VERSION` reste 43) s'ouvrent comme avant. Le banc
//!   `copies_de_vrais_projets_s_ouvrent` s'adresse à des copies de projets réels (`SHOT_PROJETS_ESSAI`).

use std::path::{Path, PathBuf};

use crate::keymap::{Keymap, DEFAULT_KEYMAP};
use crate::project;
use crate::settings::Settings;

#[test]
fn a_personal_keymap_citing_a_removed_action_is_ignored() {
    let user = r#"
        version = 1
        [[raccourci]]
        action = "fichier.importer_sboard"
        touches = ["Ctrl+Alt+F11"]
        source = "perso"
        [[raccourci]]
        action = "fichier.exporter_sboard"
        touches = ["Ctrl+Alt+F12"]
        [[raccourci]]
        action = "outil.main"
        touches = ["Ctrl+Alt+H"]
    "#;
    let map = Keymap::build(Some(user));
    // Rien ne plante ; les actions disparues sont ignorées, et le dit.
    assert!(map.bindings.iter().all(|b| b.action != "fichier.importer_sboard" && b.action != "fichier.exporter_sboard"));
    assert!(map.warnings.iter().any(|w| w.contains("fichier.importer_sboard") && w.contains("ignorée")), "{:?}", map.warnings);
    assert!(map.warnings.iter().any(|w| w.contains("fichier.exporter_sboard")), "{:?}", map.warnings);
    // Le reste du fichier personnel joue, et les raccourcis par défaut aussi.
    assert!(map.is_user("outil.main"));
    assert_eq!(map.keys_of("outil.main"), vec!["Ctrl+Alt+H".to_string()]);
    assert_eq!(map.labels_of("fichier.ouvrir"), vec!["Ctrl+O".to_string()]);
    // Aucun conflit inventé par les actions disparues.
    assert!(map.warnings.iter().all(|w| !w.contains("conflit")), "{:?}", map.warnings);
}

#[test]
fn an_old_copy_of_the_default_keymap_is_not_personal() {
    // Ce que « Modifier à la main » avait copié : le défaut d'alors, avec la source de chaque raccourci citant l'autre
    // logiciel, les commandes d'import et d'export, et un `a_verifier` que plus rien ne lit.
    let ancienne = DEFAULT_KEYMAP.replacen(
        "action = \"fichier.ouvrir\"\ntouches = [\"Ctrl+O\"]\n",
        "action = \"fichier.ouvrir\"\ntouches = [\"Ctrl+O\"]\nsource = \"ancienne source de ce raccourci\"\na_verifier = true\n",
        1,
    ) + "\n[[raccourci]]\naction = \"fichier.importer_sboard\"\ntouches = []\nsource = \"SHOT (sans touche)\"\n\n[[a_verifier]]\naction = \"mode.basculer\"\nnote = \"ancienne note\"\n";
    assert_ne!(ancienne, DEFAULT_KEYMAP, "le texte d'essai a bien été changé");
    let map = Keymap::build(Some(&ancienne));
    assert!(!map.is_user("fichier.ouvrir"), "une copie du défaut, même avec une autre source, n'est pas personnelle");
    assert!(!map.bindings.iter().any(|b| b.user), "aucune action personnelle");
    assert_eq!(map.labels_of("fichier.ouvrir"), vec!["Ctrl+O".to_string()]);
    assert!(map.warnings.iter().all(|w| !w.contains("conflit")), "{:?}", map.warnings);
}

#[test]
fn an_old_settings_file_with_removed_keys_still_loads() {
    let ancien = r#"
        copie_secours_minutes = 3
        jeu_raccourcis = "storyboard_pro"
        modele_storyboard_pro = 'C:\Modeles\vide.sboard'
        dossier_export_storyboard_pro = 'C:\Exports'
        dossier_export_montage = 'C:\Montage'
    "#;
    let s: Settings = toml::from_str(ancien).expect("un ancien réglage se lit sans erreur");
    let s = Settings::apres_lecture(s);
    assert_eq!(s.autosave_minutes, 3, "les réglages qui restent sont lus");
    assert_eq!(s.montage_export_dir, Some(PathBuf::from(r"C:\Montage")));
    // Réécrit, il ne garde aucune trace des anciennes clés.
    let ecrit = toml::to_string(&s).unwrap();
    assert!(!ecrit.contains("storyboard_pro") && !ecrit.contains("jeu_raccourcis"), "{ecrit}");
}

#[test]
fn the_project_format_did_not_change() {
    // Rien du format ne servait à l'import seul : aucune montée de version.
    assert_eq!(project::VERSION, 43);
}

/// Copie récursive d'un dossier de projet (jamais l'original).
fn copier(de: &Path, vers: &Path) {
    std::fs::create_dir_all(vers).unwrap();
    for e in std::fs::read_dir(de).unwrap().flatten() {
        let (a, b) = (e.path(), vers.join(e.file_name()));
        if a.is_dir() {
            copier(&a, &b);
        } else {
            std::fs::copy(&a, &b).unwrap();
        }
    }
}

/// Banc : ouvre une COPIE de chaque projet cité dans `SHOT_PROJETS_ESSAI` (manifestes séparés par `;`, dont le dossier
/// est copié tel quel dans un dossier temporaire) et vérifie qu'il se lit, avec ses plans, et que le premier plan se
/// compose. Lancé à la main : `cargo test --release --offline --target-dir target\msvc --lib ancien_projet_tests -- --ignored --nocapture`.
#[test]
#[ignore]
fn copies_de_vrais_projets_s_ouvrent() {
    let Ok(liste) = std::env::var("SHOT_PROJETS_ESSAI") else { return };
    for (i, manifeste) in liste.split(';').filter(|s| !s.trim().is_empty()).enumerate() {
        let manifeste = Path::new(manifeste.trim());
        let dossier = manifeste.parent().expect("dossier du projet");
        let copie = std::env::temp_dir().join(format!("shot-ancien-projet-{}-{i}", std::process::id()));
        let _ = std::fs::remove_dir_all(&copie);
        copier(dossier, &copie);
        let a_ouvrir = project::manifeste_a_ouvrir(&copie.join(manifeste.file_name().unwrap()));
        let loaded = project::load(&a_ouvrir).unwrap_or_else(|e| panic!("{} : {e}", manifeste.display()));
        let doc = loaded.doc;
        assert!(!doc.panels.is_empty(), "{} : aucun plan", manifeste.display());
        let calques: usize = doc.panels.iter().map(|p| p.layers.len()).sum();
        let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
        println!(
            "{} : {} plans, {calques} calques, {} dessins, {} sons, {} avertissement(s), {} élément(s) dans le plan 1",
            manifeste.display(),
            doc.panels.len(),
            doc.cels.len(),
            doc.sounds.len(),
            loaded.warnings.len(),
            plan.items.len()
        );
        let _ = std::fs::remove_dir_all(&copie);
    }
}
