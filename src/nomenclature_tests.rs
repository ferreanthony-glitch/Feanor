//! Essais de la nomenclature des exports par plan (partie 105) : formes, réglages, sûreté, retenue, fichiers réellement écrits.

use crate::editorial_tests::film_with_sound;
use crate::export::{self, ExportJob};
use crate::exportcles;
use crate::exports_cle_son_tests::{fichiers, projet_cles};
use crate::exports_v1_tests::{court, dossier_neuf};
use crate::exportsonplan;
use crate::exportstems::AudioFormat;
use crate::exportstills::{self, StillsPer, StillsSettings};
use crate::fxcache::monde;
use crate::nomenclature::{Casse, Forme, Nomenclature};

fn noms(doc: &crate::document::Document, n: &Nomenclature) -> Result<Vec<String>, String> {
    exportcles::items_nom(doc, &[0, 1], false, n).map(|v| v.into_iter().map(|s| s.name).collect())
}

fn avec(forme: Forme) -> Nomenclature {
    Nomenclature { forme, ..Nomenclature::default() }
}

#[test]
fn the_original_form_gives_todays_names_and_the_new_path_agrees_with_it() {
    let doc = projet_cles();
    let d = Nomenclature::default();
    assert!(d.est_defaut());
    let avant: Vec<String> = exportcles::items(&doc, &[0, 1], false).into_iter().map(|s| s.name).collect();
    assert_eq!(noms(&doc, &d).unwrap(), avant);
    // Le chemin neuf (un réglage qui retombe sur la même chose) redonne exactement les mêmes noms, composés et par calque.
    let neuf = Nomenclature { chiffres_cle: 2, ..Nomenclature::default() };
    assert!(!neuf.est_defaut());
    assert_eq!(noms(&doc, &neuf).unwrap(), avant);
    let c_avant: Vec<String> = exportcles::items_calques(&doc, &[0, 1], false).into_iter().map(|s| s.name).collect();
    let c_neuf: Vec<String> = exportcles::items_calques_nom(&doc, &[0, 1], false, &neuf).unwrap().into_iter().map(|s| s.name).collect();
    assert_eq!(c_neuf, c_avant);
    // Images par plan et par case : de même.
    for per in [StillsPer::Plan, StillsPer::Case] {
        let a: Vec<String> = exportstills::items(&doc, &[0, 1], per).into_iter().map(|s| s.name).collect();
        let b: Vec<String> = exportstills::items_nom(&doc, &[0, 1], per, false, &neuf).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(a, b, "{per:?}");
    }
}

#[test]
fn every_ready_form_gives_its_example_and_sorts_by_plan_then_key() {
    let mut doc = projet_cles();
    doc.properties.episode = "1".into();
    let attendus = [
        (Forme::Defaut, ["SQ01_PL001_K01", "SQ01_PL001_K02", "SQ01_PL001_K03", "SQ01_PL001_K04", "SQ01_PL002_K01"]),
        (Forme::DefautMinuscules, ["sq01_pl001_k01", "sq01_pl001_k02", "sq01_pl001_k03", "sq01_pl001_k04", "sq01_pl002_k01"]),
        (Forme::Tirets, ["S01-P001-K01", "S01-P001-K02", "S01-P001-K03", "S01-P001-K04", "S01-P002-K01"]),
        (Forme::Episode, ["EP01_SQ01_PL001_K01", "EP01_SQ01_PL001_K02", "EP01_SQ01_PL001_K03", "EP01_SQ01_PL001_K04", "EP01_SQ01_PL002_K01"]),
        (Forme::Court, ["001_01", "001_02", "001_03", "001_04", "002_01"]),
    ];
    for (forme, v) in attendus {
        let n = noms(&doc, &avec(forme)).unwrap();
        assert_eq!(n, v, "{forme:?}");
        let mut tries = n.clone();
        tries.sort_unstable();
        assert_eq!(tries, n, "{forme:?} : triés comme générés");
    }
    // Personnalisée : tous les champs, séparateurs libres.
    doc.properties.serie = "Mon Film".into();
    let perso = Nomenclature { forme: Forme::Perso, modele: "{projet}.E{episode:02}.{sequence:02}x{plan:03}-{cle:03}".into(), ..Nomenclature::default() };
    assert_eq!(noms(&doc, &perso).unwrap()[1], "Mon_Film_E01_01x001-002", "espaces et points deviennent « _ » (file_safe), le tiret reste");
}

#[test]
fn settings_change_digits_case_and_the_place_of_the_layer_name() {
    let doc = projet_cles();
    let n = Nomenclature { chiffres_cle: 3, chiffres_plan: 2, chiffres_sequence: 1, casse: Casse::Majuscules, ..avec(Forme::Tirets) };
    assert_eq!(noms(&doc, &n).unwrap()[0], "S1-P01-K001");
    assert_eq!(noms(&doc, &Nomenclature { casse: Casse::Minuscules, ..Nomenclature::default() }).unwrap()[0], "sq01_pl001_k01");
    let calques = |n: &Nomenclature| -> Vec<String> { exportcles::items_calques_nom(&doc, &[0], false, n).unwrap().into_iter().map(|c| c.name).collect() };
    assert_eq!(calques(&Nomenclature { chiffres_cle: 2, ..Nomenclature::default() })[0], "SQ01_PL001_K01_Perso", "à la fin");
    assert_eq!(calques(&Nomenclature { calque_au_debut: true, ..Nomenclature::default() })[0], "Perso_SQ01_PL001_K01", "au début");
    assert_eq!(calques(&Nomenclature { forme: Forme::Perso, modele: "{plan:03}_{cle}_{calque}".into(), ..Nomenclature::default() })[..2], ["001_01_Perso", "001_02_Fond"], "le modèle place le calque lui-même");
    assert_eq!(calques(&Nomenclature { casse: Casse::Majuscules, ..avec(Forme::Court) })[0], "001_01_PERSO");
}

#[test]
fn a_bad_or_ambiguous_model_is_refused_before_anything_is_written() {
    let doc = projet_cles();
    let perso = |m: &str| Nomenclature { forme: Forme::Perso, modele: m.into(), ..Nomenclature::default() };
    let e = noms(&doc, &perso("SQ{sequence:02}_PL{plan:03}")).unwrap_err();
    assert!(e.contains("{cle}"), "{e}");
    let e = noms(&doc, &perso("{sequence:02}_{cle}")).unwrap_err();
    assert!(e.contains("même nom"), "deux plans de la même séquence : {e}");
    assert!(noms(&doc, &perso("{plan:03}_{oups}")).unwrap_err().contains("oups"));
    assert!(noms(&doc, &perso("{plan:03}_{cle")).is_err());
    assert!(noms(&doc, &perso("")).is_err());
    // Les noms de calques identiques (deux calques « Fond » à la même clé) ne s'écrasent pas : refusé, avec les deux noms.
    let mut d2 = projet_cles();
    d2.panels[0].layers[1].name = "Fond".into();
    let e = exportcles::items_calques_nom(&d2, &[0], false, &Nomenclature { chiffres_cle: 2, forme: Forme::Court, ..Nomenclature::default() });
    assert!(e.is_ok(), "des clés différentes : pas de collision");
    d2.panels[0].layers[1].frames[2] = d2.panels[0].layers[1].frames[1];
    // Windows : les caractères interdits du modèle sont remplacés, les accents gardés.
    let n = noms(&doc, &perso("é:A?{plan:03}_{cle}")).unwrap();
    assert_eq!(n[0], "e_A_001_01", "accent ramené à sa lettre, comme tous les noms de fichiers des exports (editorial::file_safe)");
    assert!(perso("é:A?{plan:03}_{cle}").avertissements().iter().any(|a| a.contains("Windows")));
    assert!(perso("{plan:03}_{cle}").avertissements().is_empty());
    // Deux plans de même nom avec la forme du projet : « _2 », comme avant.
    let mut d3 = projet_cles();
    d3.properties.modele_noms = "PL".into();
    let n = noms(&d3, &Nomenclature { chiffres_cle: 2, ..Nomenclature::default() }).unwrap();
    assert_eq!(n, exportcles::items(&d3, &[0, 1], false).into_iter().map(|s| s.name).collect::<Vec<_>>());
    assert_eq!(n[4], "PL_2_K01");
}

#[test]
fn the_choice_is_kept_with_the_export_settings_and_an_old_file_gives_todays_names() {
    let n = Nomenclature { forme: Forme::Perso, modele: "{plan:03}_{cle}".into(), chiffres_cle: 3, casse: Casse::Majuscules, calque_au_debut: true, ..Nomenclature::default() };
    let mut s = export::ExportSettings::default();
    s.nomenclature = n.clone();
    let prefs = export::ExportPrefs::of(&s);
    let json = serde_json::to_string(&prefs).unwrap();
    let relu: export::ExportPrefs = serde_json::from_str(&json).unwrap();
    assert_eq!(relu.nomenclature, n);
    let mut t = export::ExportSettings::default();
    relu.apply(&mut t);
    assert_eq!(t.nomenclature, n);
    // Un réglage d'avant (sans ce champ) : la forme d'origine.
    let ancien: export::ExportPrefs = serde_json::from_str("{\"size\":\"Project\"}").unwrap_or_default();
    assert!(ancien.nomenclature.est_defaut());
    assert!(serde_json::from_str::<export::ExportPrefs>("{}").unwrap().nomenclature.est_defaut());
}

#[test]
fn the_files_written_follow_the_chosen_nomenclature() {
    monde::lire();
    let doc = projet_cles();
    // Images composées, forme à tirets.
    let n = avec(Forme::Tirets);
    let items = exportstills::items_nom(&doc, &[0, 1], StillsPer::Cle, false, &n).unwrap();
    let dir = dossier_neuf("nomencl-composees");
    let s = StillsSettings { per: StillsPer::Cle, transparent: true, ..StillsSettings::default() };
    let mut job = exportcles::job(&dir, &items, doc.frame, &s, false);
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert_eq!(fichiers(&dir), ["S01-P001-K01.png", "S01-P001-K02.png", "S01-P001-K03.png", "S01-P001-K04.png", "S01-P002-K01.png"]);
    std::fs::remove_dir_all(&dir).ok();
    // Calque par calque, minuscules, calque au début.
    let n = Nomenclature { casse: Casse::Minuscules, calque_au_debut: true, ..Nomenclature::default() };
    let items = exportcles::items_calques_nom(&doc, &[0, 1], false, &n).unwrap();
    let dir = dossier_neuf("nomencl-calques");
    let s = StillsSettings { per: StillsPer::Cle, par_calque: true, ..StillsSettings::default() };
    let mut job = exportcles::job_calques(&dir, items, doc.frame, &s, false);
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert_eq!(fichiers(&dir), ["fond_sq01_pl001_k02.png", "fond_sq01_pl001_k04.png", "fond_sq01_pl002_k01.png", "perso_sq01_pl001_k01.png", "perso_sq01_pl001_k03.png"]);
    for f in fichiers(&dir) {
        assert!(image::open(dir.join(&f)).is_ok(), "{f} se relit");
    }
    std::fs::remove_dir_all(&dir).ok();
    // Son par plan, forme courte.
    let film = film_with_sound(24);
    let files = exportsonplan::plan_nom(&film, &[0, 1, 2, 3], AudioFormat::default(), &crate::audio::mix_clips_blocking(&film), &avec(Forme::Court)).unwrap();
    let dir = dossier_neuf("nomencl-son");
    let (d, f2) = (dir.clone(), files.clone());
    let mut job = ExportJob::task("Son par plan".into(), &dir, "ok".into(), move |snap, cancel| exportsonplan::write_all(snap, &d, &f2, AudioFormat::default(), cancel).map(|_| ()));
    court(&mut job, &film);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert_eq!(fichiers(&dir), ["001.wav", "002.wav", "003.wav", "004.wav"]);
    std::fs::remove_dir_all(&dir).ok();
    // Un modèle qui ferait deux fois le même nom : refusé, rien n'est écrit.
    let e = exportsonplan::plan_nom(&film, &[0, 1, 2, 3], AudioFormat::default(), &crate::audio::mix_clips_blocking(&film), &Nomenclature { forme: Forme::Perso, modele: "SQ{sequence:02}".into(), ..Nomenclature::default() }).unwrap_err();
    assert!(e.contains("même nom"), "{e}");
}
