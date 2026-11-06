//! Essais du modèle de noms des plans (`nommage.rs`) et de ses effets sur les exports.

use crate::document::Document;
use crate::editorial::{self, Options};
use crate::editorial_tests::film;
use crate::exportstills::{self, StillsPer};
use crate::nommage::{self, Contexte, Jeton, Piece};
use crate::numerotation::Num;

fn cx() -> Contexte {
    Contexte {
        projet: "Mon Film".into(),
        episode: "7".into(),
        sequence: String::new(),
        sequence_num: Some(Num::new(3)),
        plan_num: Some(Num::parse("12A").unwrap()),
        version: 2,
        case: Some(4),
        cle: None,
        calque: Some("Fond/Ciel".into()),
        date: "20261030".into(),
    }
}

fn rempli(modele: &str) -> String {
    nommage::remplir(&nommage::analyser(modele).unwrap(), &cx())
}

#[test]
fn tokens_widths_prefixes_and_separators() {
    assert_eq!(rempli("SQ{sequence:02}_PL{plan:03}"), "SQ03_PL012A");
    assert_eq!(rempli("{sequence}-{plan}"), "3-12A");
    assert_eq!(rempli("{projet}_EP{episode:02}_SQ{sequence:03}_SH{plan:04}_v{version:03}"), "Mon Film_EP07_SQ003_SH0012A_v002");
    assert_eq!(rempli("{date}.{case:02}"), "20261030.04");
    assert_eq!(rempli("{calque}"), "Fond_Ciel", "le nom du calque est nettoyé");
    assert_eq!(rempli("{{{plan}}}"), "{12A}", "les accolades doublées sont écrites telles quelles");
    // Un épisode qui n'est pas un nombre est gardé tel quel.
    let mut c = cx();
    c.episode = "pilote".into();
    assert_eq!(nommage::remplir(&nommage::analyser("EP{episode:02}").unwrap(), &c), "EPpilote");
}

#[test]
fn bad_templates_are_explained() {
    for (m, mot) in [("{inconnu}", "inconnu"), ("SQ{sequence", "refermée"), ("a}b", "accolade"), ("{plan:x}", "largeur"), ("", "vide"), ("{plan:123}", "largeur")] {
        let e = nommage::analyser(m).unwrap_err();
        assert!(e.contains(mot), "{m} : {e}");
    }
    assert_eq!(nommage::analyser("{PLAN:03}").unwrap(), vec![Piece::Jeton { jeton: Jeton::Plan, largeur: 3 }], "majuscules permises");
}

#[test]
fn the_default_model_gives_the_names_of_before_for_every_plan() {
    let mut doc = film(24);
    for locked in [true, false] {
        if !locked {
            doc.unlock_numbering();
        }
        for p in 0..doc.panels.len() {
            assert_eq!(doc.nom_de_plan(p), doc.plan_full_ref(p), "plan {p}, verrouillé {locked}");
            assert_eq!(editorial::clip_name(&doc, p), doc.plan_full_ref(p));
        }
    }
    assert_eq!(doc.nom_de_plan(0), "SQ01_PL001");
    // Un plan inséré porte sa lettre : 12A.
    doc.lock_numbering();
    let i = doc.add_scene_after(0);
    assert_eq!(doc.nom_de_plan(i), "SQ01_PL001A");
}

#[test]
fn versions_and_cases_keep_the_old_suffix_rule_without_their_token() {
    let mut doc = film(24);
    let avant = doc.plan_full_ref(1);
    doc.nouvelle_version(1, 1);
    let id = doc.panels[1].id;
    assert!(doc.nombre_de_versions(id) > 1);
    {
        assert_eq!(doc.nom_de_plan(1), format!("{avant}_V{}", doc.numero_de_version(id)));
        // Avec {version} dans le modèle, plus de suffixe en plus.
        doc.properties.modele_noms = "{plan:03}_v{version:02}".into();
        assert_eq!(doc.nom_de_plan(1), format!("002_v{:02}", doc.numero_de_version(id)));
    }
    doc.properties.modele_noms.clear();
    assert_eq!(doc.nom_de_plan_avec(0, Some(1), None), "SQ01_PL001");
    assert_eq!(doc.nom_de_plan_avec(0, Some(3), None), "SQ01_PL001_C03");
    doc.properties.modele_noms = "{plan}_c{case:02}".into();
    assert_eq!(doc.nom_de_plan_avec(0, Some(3), None), "1_c03");
}

#[test]
fn a_studio_model_names_the_plans() {
    let mut doc = film(24);
    doc.properties.serie = "Brawlidays".into();
    doc.properties.episode = "2".into();
    doc.properties.modele_noms = nommage::MODELES_PRETS[1].1.into();
    assert_eq!(doc.nom_de_plan(0), "Brawlidays_EP02_SQ001_SH0001_v001");
    assert_eq!(doc.nom_de_plan(3), "Brawlidays_EP02_SQ002_SH0004_v001");
    // Sans nom de série, le titre du projet.
    doc.properties.serie.clear();
    doc.properties.title = "Mon titre".into();
    assert!(doc.nom_de_plan(0).starts_with("Mon titre_EP02"));
}

#[test]
fn check_warns_about_duplicates_forbidden_characters_and_unreadable_models() {
    let doc = film(24);
    let v = nommage::verifier(&doc, "");
    assert!(v.erreur.is_none() && v.avertissements.is_empty(), "{v:?}");
    assert_eq!(v.exemples, ["SQ01_PL001", "SQ02_PL004"]);
    // Sans séquence : le plan 1 de la séquence 2 aurait le même nom que... ici les plans sont numérotés de suite : pas de double.
    // Un modèle sans plan : tous les plans d'une séquence se ressemblent.
    let v = nommage::verifier(&doc, "SQ{sequence:02}");
    assert!(v.avertissements.iter().any(|a| a.contains("nom déjà pris")), "{v:?}");
    let v = nommage::verifier(&doc, "{plan}:{sequence}");
    assert!(v.avertissements.iter().any(|a| a.contains("Windows refuse")), "{v:?}");
    let v = nommage::verifier(&doc, "{plan");
    assert!(v.erreur.is_some());
    let v = nommage::verifier(&doc, "CON");
    assert!(v.avertissements.iter().any(|a| a.contains("réservé")), "{v:?}");
    // Les modèles prêts se lisent tous et ne produisent aucun avertissement sur ce film.
    for (nom, m) in nommage::MODELES_PRETS {
        let v = nommage::verifier(&doc, m);
        assert!(v.erreur.is_none() && v.avertissements.is_empty(), "{nom} : {v:?}");
    }
    // Windows ne distingue pas les majuscules : « a » et « A » comptent pour le même nom.
    let v = nommage::verifier(&doc, "x{sequence}");
    assert!(!v.avertissements.is_empty());
}

#[test]
fn an_unreadable_saved_model_falls_back_to_the_default() {
    let mut doc = film(24);
    doc.properties.modele_noms = "{casse".into();
    assert_eq!(doc.nom_de_plan(0), "SQ01_PL001");
}

#[test]
fn every_export_that_names_files_by_plan_uses_the_model() {
    let mut doc = film(24);
    doc.properties.serie = "Serie".into();
    doc.properties.episode = "3".into();
    doc.properties.modele_noms = "{projet}_E{episode:02}_{sequence:02}_{plan:03}".into();
    let attendu = ["Serie_E03_01_001", "Serie_E03_01_002", "Serie_E03_01_003", "Serie_E03_02_004"];
    // Vidéos par plan, clips du montage.
    let plan = editorial::build_plan(&doc, &Options { sound: false, ..Options::default() }, 0, "Film").unwrap();
    let noms: Vec<&str> = plan.clips.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(noms, attendu);
    assert!(plan.clips[0].file.starts_with("media/Serie_E03_01_001."), "{}", plan.clips[0].file);
    // Images par plan.
    let items = exportstills::items(&doc, &[0, 1, 2, 3], StillsPer::Plan);
    assert_eq!(items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), attendu);
    // Fiche technique : la référence.
    assert_eq!(crate::fiche::reference(&doc, 3), attendu[3]);
    // Calques séparés : le dossier suit le modèle.
    let info = crate::exportlayers::plans_info(&doc, &[0, 3], false);
    assert_eq!(info.iter().map(|i| i.folder.as_str()).collect::<Vec<_>>(), [attendu[0], attendu[3]]);
}

#[test]
fn layer_files_follow_the_calque_token() {
    let mut doc: Document = film(24);
    doc.properties.modele_noms = "{plan:03}_{calque}".into();
    let info = crate::exportlayers::plans_info(&doc, &[0], false);
    for (_, stem) in &info[0].layers {
        assert!(stem.starts_with("001_"), "{stem}");
    }
}
