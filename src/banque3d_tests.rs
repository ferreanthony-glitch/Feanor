//! Essais de la banque 3D (partie 181) : le catalogue rangé dans le programme se lit, chaque modèle et chaque aperçu qu'il cite
//! y sont (identiques à `assets/banque3d/`) et se lisent ; l'onglet « 3D » de la Bibliothèque montre 45 modèles en cinq
//! familles ; un modèle de la banque passe par le même chemin qu'un fichier importé (budget, fenêtre « Ce décor est lourd »).

use std::path::Path;

use crate::banque3d::{self, FAMILLES};
use crate::decor3d_tests::{attendre, banc_avec_budget, budget_large};
use crate::decor3dbudget::BudgetMachine;

fn racine() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("banque3d")
}

#[test]
fn le_catalogue_se_lit_et_chaque_fichier_cite_est_range_et_se_lit() {
    let modeles = banque3d::modeles();
    assert_eq!(modeles.len(), 45);
    // Le même que sur le disque.
    let disque = banque3d::lire_catalogue(&std::fs::read_to_string(racine().join("catalogue.json")).unwrap()).unwrap();
    assert_eq!(disque, modeles);
    for m in modeles {
        assert!(FAMILLES.contains(&m.famille.as_str()), "{}", m.famille);
        assert_eq!((m.auteur.as_str(), m.licence.as_str()), ("Kenney (www.kenney.nl)", "CC0 1.0"), "{}", m.nom);
        let glb = m.glb().unwrap_or_else(|| panic!("{} n'est pas rangé dans le programme", m.fichier));
        assert_eq!(glb, std::fs::read(racine().join(&m.fichier)).unwrap().as_slice(), "{} : rangé identique au disque", m.fichier);
        assert_eq!(glb.len() as u64, m.octets, "{}", m.fichier);
        let (scene, mesures) = crate::decor3dgltf::lire(glb, None).unwrap_or_else(|e| panic!("{} : {e}", m.fichier));
        assert_eq!(mesures.triangles as u64, m.triangles, "{}", m.fichier);
        assert!(scene.triangles() > 0);
        let png = m.png().unwrap_or_else(|| panic!("{} n'est pas rangé", m.apercu));
        assert_eq!(png, std::fs::read(racine().join(&m.apercu)).unwrap().as_slice());
        let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).unwrap_or_else(|e| panic!("{} : {e}", m.apercu));
        assert!(img.width() > 0 && img.height() > 0);
    }
    // Rien de rangé en plus de ce que le catalogue cite.
    assert_eq!(crate::banque3dfichiers::FICHIERS.len(), 90);
}

#[test]
fn un_catalogue_d_un_autre_format_ou_plus_recent_est_refuse() {
    assert!(banque3d::lire_catalogue(r#"{"format": "autre", "version": 1, "modeles": []}"#).is_err());
    assert!(banque3d::lire_catalogue(r#"{"format": "feanor-banque3d", "version": 2, "modeles": []}"#).unwrap_err().contains("plus récente"));
    assert!(banque3d::lire_catalogue("pas du json").is_err());
}

#[test]
fn l_onglet_montre_45_modeles_en_cinq_familles_et_la_recherche_trie() {
    let f = banque3d::par_famille("");
    assert_eq!(f.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), FAMILLES.to_vec(), "les cinq familles, dans l'ordre");
    assert_eq!(f.iter().map(|(_, r)| r.len()).sum::<usize>(), 45);
    assert_eq!(f.iter().map(|(_, r)| r.len()).collect::<Vec<_>>(), vec![21, 8, 7, 6, 3]);
    // La recherche : dans le nom, la famille ou le kit, sans tenir compte des majuscules ; les familles vides disparaissent.
    let routes = banque3d::par_famille("ROAD");
    assert!(routes.iter().all(|(n, _)| n == "Routes"), "{routes:?}");
    assert!(banque3d::par_famille("nature").iter().map(|(_, r)| r.len()).sum::<usize>() == 21);
    assert!(banque3d::par_famille("zzz introuvable").is_empty());
    // La bulle : nom, triangles, kit, licence.
    let m = &banque3d::modeles()[f[0].1[0]];
    let bulle = m.bulle();
    assert!(bulle.contains(&m.nom_lisible()) && bulle.contains("triangles") && bulle.contains(&m.kit) && bulle.contains("CC0 · Kenney"), "{bulle}");
}

#[test]
fn chaque_modele_de_la_banque_s_analyse_comme_un_fichier_importe() {
    for (i, m) in banque3d::modeles().iter().enumerate() {
        let a = banque3d::analyser(i).unwrap_or_else(|e| panic!("{} : {e}", m.nom));
        assert_eq!(a.mesures.triangles as u64, m.triangles);
        assert!(a.origine.starts_with("Banque 3D") && a.origine.contains(&m.kit));
        assert_eq!(a.empreinte, crate::decor3d::empreinte(&a.glb));
    }
}

#[test]
fn un_modele_de_la_banque_passe_par_le_controle_de_poids() {
    // Sous le budget : posé dans le décor, devant la caméra, aimanté au sol ; une étape d'annulation.
    let mut b = banc_avec_budget(budget_large());
    let etapes = b.app.history.undo_len();
    b.app.decor3d_ajouter_banque(0, None);
    attendre(&mut b);
    let d = b.app.doc.decor_du_plan(0).expect("posé");
    let o = &d.objets[0];
    assert_eq!(o.nom, banque3d::modeles()[0].nom_lisible());
    assert!(o.source.starts_with("Banque 3D"), "{}", o.source);
    assert!(o.aimante);
    let sol = crate::espace3d::Monde::of(&b.app.doc).sol_y();
    assert!((o.bornes_monde().0[1] - sol).abs() < 1e-3, "sur le sol");
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    // Un point du sol donné (lâché sur la vue Espace 3D) : l'origine y est.
    b.app.decor3d_ajouter_banque(1, Some([2.0, -3.0]));
    attendre(&mut b);
    let o = b.app.doc.decor_du_plan(0).unwrap().objets.last().unwrap().clone();
    assert!((o.place.position[0] - 2.0).abs() < 1e-4 && (o.place.position[2] + 3.0).abs() < 1e-4, "{:?}", o.place);
    // Au-delà du budget de la machine : la fenêtre « Ce décor est lourd », rien n'est posé avant le choix.
    let mut b = banc_avec_budget(BudgetMachine { triangles: 10, ..budget_large() });
    b.app.decor3d_ajouter_banque(0, None);
    attendre(&mut b);
    assert!(b.app.decor3d.fenetre_ouverte(), "le même contrôle qu'un fichier importé");
    assert!(b.app.doc.decor_du_plan(0).is_none());
}

#[test]
fn le_credit_de_kenney_est_dans_a_propos() {
    assert!(banque3d::CREDIT.contains("Kenney") && banque3d::CREDIT.contains("CC0"));
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("aproposui.rs")).unwrap();
    assert!(source.contains("banque3d::CREDIT"));
}

#[test]
fn every_model_of_the_bank_has_a_short_french_name_of_its_own() {
    let modeles = banque3d::modeles();
    assert_eq!(modeles.len(), banque3d::NOMS_FR.len(), "un modèle ajouté au catalogue reçoit son nom français");
    let mut vus = std::collections::HashSet::new();
    for m in modeles {
        let fr = banque3d::NOMS_FR.iter().find(|(nom, _)| *nom == m.nom).map(|(_, fr)| *fr).unwrap_or_else(|| panic!("pas de nom français pour {}", m.nom));
        assert_eq!(m.nom_lisible(), fr);
        assert!(fr.chars().count() <= 18, "« {fr} » ne tient pas sous une vignette");
        assert!(vus.insert(fr), "« {fr} » est donné deux fois");
        // La recherche trouve le nom français comme le nom du fichier.
        assert!(m.correspond(fr) && m.correspond(&m.nom), "{fr}");
    }
}
