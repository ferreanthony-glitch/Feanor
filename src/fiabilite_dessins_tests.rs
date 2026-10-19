//! Dessins abîmés : un fichier de dessin manquant, tronqué, d'une autre taille, au contenu absurde.
//! Le projet s'ouvre sans ce dessin, avec un avertissement qui le nomme ; son fichier n'est ni lu
//! de travers (un en-tête qui annonce des dizaines de gigaoctets), ni effacé par l'enregistrement
//! qui suit ; et rien ne plante ni ne bloque, quoi qu'il y ait dedans (essais à la louche).

use std::collections::HashMap;
use std::time::Duration;

use crate::document::{Cel, Document, Layer, LayerKind};
use crate::fiabilite_entrees_tests::muter_fichier;
use crate::fiabilite_fixture::{arbre, charger, dossier, projet_enregistre, racine, Lcg};
use crate::project;
use crate::raster::{premultiply, Raster, TILE};

/// Un PNG valide de `w` × `h` pixels, tout transparent.
fn png_vide(w: u32, h: u32) -> Vec<u8> {
    crate::io::encode_png(&vec![0u8; (w * h * 4) as usize], w as usize, h as usize).expect("png")
}

/// Un PNG de 1 × 1 dont l'en-tête annonce `w` × `h` (la somme de contrôle est refaite).
fn png_demesure(w: u32, h: u32) -> Vec<u8> {
    let mut b = png_vide(1, 1);
    b[16..20].copy_from_slice(&w.to_be_bytes());
    b[20..24].copy_from_slice(&h.to_be_bytes());
    let mut crc = flate2::Crc::new();
    crc.update(&b[12..29]);
    b[29..33].copy_from_slice(&crc.sum().to_be_bytes());
    b
}

#[test]
fn missing_and_damaged_drawings_are_reported_and_their_files_are_never_deleted() {
    let (f, manifeste) = projet_enregistre("dessins-abimes");
    let dessins = racine(&manifeste).join("dessins");
    // Des dessins du projet d'essai, dans l'ordre de leurs numéros (HashMap : l'ordre ne l'est pas).
    let mut bitmaps: Vec<u64> = f.doc.cels.iter().filter(|(_, c)| c.vector.is_none() && c.fond.is_none()).map(|(id, _)| *id).collect();
    let mut vecteurs: Vec<u64> = f.doc.cels.iter().filter(|(_, c)| c.vector.is_some()).map(|(id, _)| *id).collect();
    bitmaps.sort_unstable();
    vecteurs.sort_unstable();
    let (manquant_png, manquant_json, tronque, illisible, mauvaise_taille, demesure, json_abime) = (bitmaps[0], vecteurs[0], bitmaps[1], bitmaps[2], bitmaps[3], bitmaps[4], vecteurs[1]);
    let png = |id: u64| dessins.join(format!("{id}.png"));
    let json = |id: u64| dessins.join(format!("{id}.json"));
    std::fs::remove_file(png(manquant_png)).expect("effacement");
    std::fs::remove_file(json(manquant_json)).expect("effacement");
    let octets = std::fs::read(png(tronque)).expect("lecture");
    std::fs::write(png(tronque), &octets[..octets.len() / 2]).expect("écriture");
    std::fs::write(png(illisible), b"ceci n'est pas une image").expect("écriture");
    std::fs::write(png(mauvaise_taille), png_vide(10, 10)).expect("écriture");
    std::fs::write(png(demesure), png_demesure(70_000, 70_000)).expect("écriture");
    std::fs::write(json(json_abime), b"{\"elements\": [ {\"type\": \"Stroke\", \"points\": ").expect("écriture");
    let avant = arbre(racine(&manifeste));

    let mut l = project::load(&manifeste).expect("le projet s'ouvre malgré les dessins abîmés");
    assert_eq!(l.warnings.len(), 7, "un avertissement par dessin : {:#?}", l.warnings);
    for id in [manquant_png, manquant_json, tronque, illisible, mauvaise_taille, demesure, json_abime] {
        assert!(l.warnings.iter().any(|w| w.contains(&format!("dessin {id} "))), "pas d'avertissement pour le dessin {id} : {:?}", l.warnings);
        assert!(!l.doc.cels.contains_key(&id), "le dessin {id} est resté dans le projet");
        assert!(l.doc.panels.iter().flat_map(|p| &p.layers).all(|ly| !ly.frames.contains(&Some(id))), "une image montre encore le dessin {id}");
    }
    assert!(l.warnings.iter().any(|w| w.contains("introuvable")), "dessin manquant : message clair");
    assert!(l.warnings.iter().any(|w| w.contains("taille différente")), "dessin de mauvaise taille : message clair");
    assert!(l.warnings.iter().any(|w| w.contains("démesurée")), "en-tête démesuré : message clair");
    assert_eq!(arbre(racine(&manifeste)), avant, "ouvrir ne touche à aucun fichier");

    // Enregistrer le projet ouvert ne détruit pas les fichiers qu'il n'a pas pu lire.
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    let apres = arbre(racine(&manifeste));
    for id in [tronque, illisible, mauvaise_taille, demesure] {
        let nom = format!("dessins/{id}.png");
        assert_eq!(apres.get(&nom), avant.get(&nom), "le fichier du dessin {id} a été touché par l'enregistrement");
    }
    let nom = format!("dessins/{json_abime}.json");
    assert_eq!(apres.get(&nom), avant.get(&nom), "le JSON abîmé a été touché");
    // Le projet enregistré ne cite plus ces dessins (leurs images sont vides) : il se rouvre sans
    // avertissement, et leurs fichiers attendent toujours dans le dossier.
    let l2 = project::load(&manifeste).expect("réouverture");
    assert!(l2.warnings.is_empty(), "{:#?}", l2.warnings);
}

/// Un petit projet de deux dessins bitmap : des troncatures et des octets changés dans un PNG.
#[test]
fn a_truncated_or_corrupted_drawing_png_never_crashes_the_reader() {
    let travail = dossier("fuzz-png");
    let mut doc = Document::with_frame(64, 36, 24);
    let mut ids = Vec::new();
    for s in 0..2u32 {
        let mut r = Raster::new(doc.width, doc.height);
        for y in 10..50 {
            for x in 10..90 {
                let i = r.tile_index_of(x, y);
                r.tile_mut(i)[(y % TILE) * TILE + x % TILE] = premultiply((x * 3) as u8, (y * 5) as u8, (s * 100) as u8, 200);
            }
        }
        ids.push(doc.add_cel(Cel::bitmap(r)));
    }
    let mut l = Layer::new("B", LayerKind::Bitmap);
    l.frames = vec![Some(ids[0]), Some(ids[1])];
    doc.panels[0].layers = vec![l];
    let manifeste = travail.join("Film.pelure");
    project::save(&doc, &manifeste, &mut HashMap::new()).expect("enregistrement");
    let fichier = racine(&manifeste).join("dessins").join(format!("{}.png", ids[0]));
    let original = std::fs::read(&fichier).expect("PNG");
    let mut rng = Lcg(77);
    let mut cas: Vec<(String, Vec<u8>)> = Vec::new();
    for _ in 0..60 {
        let coupe = rng.sous(original.len());
        cas.push((format!("coupé à {coupe} octets"), original[..coupe].to_vec()));
    }
    for _ in 0..60 {
        let mut b = original.clone();
        for _ in 0..1 + rng.sous(4) {
            let i = rng.sous(b.len());
            b[i] ^= 1 << rng.sous(8);
        }
        cas.push(("bits changés".to_string(), b));
    }
    // L'en-tête annonce une feuille énorme : refusé sans demander la mémoire.
    cas.push(("en-tête démesuré".to_string(), png_demesure(60_000, 60_000)));
    cas.push(("en-tête de largeur nulle".to_string(), png_demesure(0, 5)));
    let (mut ouverts, mut avertis) = (0, 0);
    for (nom, octets) in cas {
        std::fs::write(&fichier, &octets).expect("écriture");
        match charger(&manifeste, Duration::from_secs(30)) {
            Ok(Ok(l)) => {
                if l.warnings.is_empty() {
                    ouverts += 1;
                } else {
                    avertis += 1;
                    assert!(l.warnings.iter().any(|w| w.contains(&format!("dessin {} ", ids[0]))), "{nom} : {:?}", l.warnings);
                }
            }
            Ok(Err(m)) => panic!("{nom} : le projet est refusé pour un dessin abîmé (« {m} »)"),
            Err(p) => panic!("{nom} : la lecture {p}"),
        }
        assert_eq!(std::fs::read(&fichier).expect("PNG"), octets, "{nom} : la lecture a modifié le fichier");
    }
    println!("FIABILITE PNG abîmés : {ouverts} lus quand même, {avertis} signalés");
    assert!(avertis > 50, "la plupart des PNG abîmés doivent être signalés ({avertis})");
}

/// Un dessin vectoriel dont des valeurs changent au hasard — des coordonnées de 3,4e38 comprises :
/// la lecture finit, quoi qu'il y ait dans le fichier (elle bloquait à rejouer le trait).
#[test]
fn a_vector_drawing_with_random_values_changed_never_crashes_or_hangs_the_reader() {
    let (f, manifeste) = projet_enregistre("fuzz-vecteur");
    // Le dessin texturé (avec trace) et celui des formes (remplissage, trait lisse), dans l'ordre.
    let mut cibles: Vec<u64> = f.doc.cels.iter().filter(|(_, c)| c.vector.as_ref().is_some_and(|v| v.elements.len() >= 2)).map(|(id, _)| *id).collect();
    cibles.sort_unstable();
    assert!(cibles.len() >= 2);
    for (k, id) in cibles.into_iter().take(3).enumerate() {
        muter_fichier(&manifeste, &format!("dessins/{id}.json"), 60, 20 + k as u64);
    }
}

/// Le cas trouvé par l'essai précédent, à part : une coordonnée de 3,4e38 dans un trait.
#[test]
fn a_stroke_with_an_enormous_coordinate_is_refused_not_replayed_forever() {
    let (f, manifeste) = projet_enregistre("coordonnee-enorme");
    let mut cibles: Vec<u64> = f.doc.cels.iter().filter(|(_, c)| c.vector.as_ref().is_some_and(|v| v.elements.iter().any(|e| matches!(e, crate::vector::Element::Stroke { brush: Some(_), .. })))).map(|(id, _)| *id).collect();
    cibles.sort_unstable();
    let id = cibles[0];
    let chemin = racine(&manifeste).join("dessins").join(format!("{id}.json"));
    for (adresse, valeur) in [("/elements/0/points/5/x", 3.4e38), ("/elements/0/points/3/y", -1e30), ("/elements/0/points/2/r", 1e30), ("/elements/0/points/2/r", -5.0)] {
        let mut v = crate::fiabilite_fixture::lire_json(&chemin);
        *v.pointer_mut(adresse).expect("point") = serde_json::Number::from_f64(valeur).map_or(serde_json::Value::Null, serde_json::Value::Number);
        crate::fiabilite_fixture::ecrire_json(&chemin, &v);
        let avant = arbre(racine(&manifeste));
        let l = charger(&manifeste, Duration::from_secs(20)).unwrap_or_else(|e| panic!("{adresse} = {valeur} : la lecture {e}")).expect("le projet s'ouvre");
        assert!(l.warnings.iter().any(|w| w.contains(&format!("dessin {id} "))), "{adresse} = {valeur} : le dessin devrait être signalé : {:?}", l.warnings);
        assert_eq!(arbre(racine(&manifeste)), avant, "la lecture ne touche à rien");
    }
}
