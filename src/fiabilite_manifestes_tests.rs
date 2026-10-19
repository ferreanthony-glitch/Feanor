//! Manifestes abîmés : un manifeste vide, tronqué, fait d'octets quelconques, d'une autre sorte de
//! fichier ou de valeurs absurdes (feuille démesurée) donne un message clair en français, et la
//! lecture ne touche à aucun fichier du dossier.

use std::time::Duration;

use serde_json::Value;

use crate::fiabilite_fixture::{arbre, charger, projet_enregistre, racine};
use crate::project;

#[test]
fn damaged_manifests_give_a_clear_message_and_never_touch_the_folder() {
    let (_f, manifeste) = projet_enregistre("manifestes-abimes");
    let original = std::fs::read(&manifeste).expect("manifeste");
    let avant = arbre(racine(&manifeste));
    let valeur = |f: &dyn Fn(&mut Value)| {
        let mut m: Value = serde_json::from_slice(&original).expect("JSON");
        f(&mut m);
        serde_json::to_vec_pretty(&m).expect("écriture")
    };
    let cas: Vec<(&str, Vec<u8>, &str)> = vec![
        ("fichier vide", Vec::new(), "manifeste illisible"),
        ("coupé en deux", original[..original.len() / 2].to_vec(), "manifeste illisible"),
        ("coupé au premier octet", original[..1].to_vec(), "manifeste illisible"),
        ("octets quelconques", (0..400u32).map(|i| (i * 37 % 251) as u8).collect(), "manifeste illisible"),
        ("objet vide", b"{}".to_vec(), "manifeste illisible"),
        ("pas un projet SHOT", valeur(&|m| m["format"] = Value::from("autre-chose")), "n'est pas un projet SHOT"),
        ("version plus récente", valeur(&|m| m["version"] = Value::from(project::VERSION + 5)), "plus récente"),
        ("feuille de taille nulle", valeur(&|m| m["largeur"] = Value::from(0)), "taille d'image invalide"),
        ("feuille démesurée", valeur(&|m| m["hauteur"] = Value::from(4_000_000_000u64)), "démesurée"),
        ("aucune séquence", valeur(&|m| m["sequences"] = Value::Array(Vec::new())), "aucun plan"),
    ];
    for (nom, octets, attendu) in cas {
        std::fs::write(&manifeste, &octets).expect("écriture");
        match charger(&manifeste, Duration::from_secs(60)).unwrap_or_else(|e| panic!("manifeste « {nom} » : la lecture {e}")) {
            Err(message) => assert!(message.contains(attendu), "manifeste « {nom} » : message « {message} » sans « {attendu} »"),
            Ok(_) => panic!("manifeste « {nom} » : la lecture aurait dû être refusée"),
        }
        let mut apres = arbre(racine(&manifeste));
        let mut attendu_arbre = avant.clone();
        apres.remove("Film.pelure");
        attendu_arbre.remove("Film.pelure");
        assert_eq!(apres, attendu_arbre, "manifeste « {nom} » : un fichier a changé ou disparu");
    }
    // Un manifeste absent : un message qui dit où il devrait être.
    std::fs::remove_file(&manifeste).expect("effacement");
    match charger(&manifeste, Duration::from_secs(30)).expect("lecture") {
        Err(m) => assert!(m.contains("introuvable") && m.contains("Film.pelure"), "{m}"),
        Ok(_) => panic!("sans manifeste, rien ne s'ouvre"),
    }
    // Avec un BOM devant (un éditeur de texte), il s'ouvre.
    let mut avec_bom = vec![0xEF, 0xBB, 0xBF];
    avec_bom.extend_from_slice(&original);
    std::fs::write(&manifeste, &avec_bom).expect("écriture");
    let l = charger(&manifeste, Duration::from_secs(60)).expect("lecture").expect("un manifeste avec BOM s'ouvre");
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
}
