//! Tests du noyau (10a) : ce que l'appli compagnon Tauri voit et fait d'un projet.

use crate::noyau::Projet;

#[test]
fn a_new_project_is_summed_up() {
    let p = Projet::nouveau(1920, 1080, 24);
    let r = p.resume();
    assert_eq!((r.cadre, r.fps, r.nom.as_str(), r.modifie), ([1920, 1080], 24, "Sans titre", false));
    assert_eq!(r.sequences.len(), 1);
    assert_eq!(r.sequences[0].scenes.len(), 1);
    let panneaux = &r.sequences[0].scenes[0].panneaux;
    assert_eq!(panneaux.len(), 1);
    assert_eq!(panneaux[0].titre, "Plan 1");
    assert_eq!(panneaux[0].debut, "0:00:00");
    assert_eq!(r.images, panneaux[0].images);
}

#[test]
fn panels_are_added_moved_written_and_removed() {
    let mut p = Projet::nouveau(320, 180, 24);
    let b = p.ajouter_panneau(0).unwrap();
    assert_eq!(b, 1);
    p.modifier_textes(1, "Il court".into(), "« Vite ! »".into(), "plan large".into()).unwrap();
    assert_eq!(p.regler_duree(1, 36).unwrap(), 36);
    assert!(p.resume().modifie);
    // Le second passe devant.
    let id = p.doc.panels[1].id;
    assert_eq!(p.deplacer_panneau(1, 0).unwrap(), 0);
    // Un plan par scène (13a).
    let r = p.resume();
    let plans: Vec<_> = r.sequences[0].scenes.iter().map(|s| &s.panneaux[0]).collect();
    assert!(r.sequences[0].scenes.iter().all(|s| s.panneaux.len() == 1));
    assert_eq!((plans[0].id, plans[0].action.as_str(), plans[0].images), (id, "Il court", 36));
    assert_eq!(plans[1].debut, "0:01:12", "le second commence après 36 images");
    assert_eq!(p.dupliquer_panneau(0).unwrap(), 1);
    assert_eq!(p.doc.panels[1].dialogue, "« Vite ! »");
    p.supprimer_panneau(1).unwrap();
    p.supprimer_panneau(1).unwrap();
    assert!(p.supprimer_panneau(0).is_err(), "on garde au moins un panneau");
    assert!(p.ajouter_panneau(9).is_err());
}

#[test]
fn thumbnails_are_png_at_the_asked_width() {
    let p = Projet::nouveau(320, 180, 24);
    let png = p.vignette_png(0, 192).unwrap();
    let img = image::load_from_memory(&png).unwrap();
    assert_eq!((img.width(), img.height()), (192, 108));
    // Papier blanc.
    assert_eq!(img.to_rgba8().get_pixel(10, 10).0, [255, 255, 255, 255]);
}

/// Projet d'exemple pour essayer l'appli compagnon : quatre plans dessinés.
/// `PELURE_EXEMPLE=<dossier>\Exemple.pelure cargo test --release ecrire_un_projet_exemple -- --ignored`
#[test]
#[ignore]
fn ecrire_un_projet_exemple() {
    use crate::document::{Layer, LayerKind};
    use eframe::egui::Color32;
    let manifest = std::path::PathBuf::from(std::env::var("PELURE_EXEMPLE").expect("PELURE_EXEMPLE"));
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    let mut p = Projet::nouveau(640, 360, 24);
    for i in 0..3 {
        p.ajouter_panneau(i).unwrap();
    }
    let couleurs = [Color32::from_rgb(220, 60, 50), Color32::from_rgb(50, 120, 220), Color32::from_rgb(60, 170, 90), Color32::from_rgb(240, 170, 40)];
    let textes = ["Le bus arrive au loin.", "Gros plan : le chauffeur.", "La porte s'ouvre.", "Tout le monde monte."];
    for (i, (c, t)) in couleurs.iter().zip(textes).enumerate() {
        p.doc.panels[i].layers = vec![Layer::new("Dessin", LayerKind::Bitmap)];
        let cel = p.doc.new_cel(LayerKind::Bitmap);
        let r = &mut p.doc.cels.get_mut(&cel).unwrap().raster;
        // Un disque qui se décale d'un panneau à l'autre (feuille 1280×720, cadre au centre).
        let (cx, cy, rayon) = (480.0 + 110.0 * i as f32, 360.0, 90.0);
        for y in 260..460 {
            for x in 380..900 {
                if ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() <= rayon {
                    let idx = r.tile_index_of(x, y);
                    r.tile_mut(idx)[(y % 64) * 64 + x % 64] = *c;
                }
            }
        }
        let n = p.doc.panels[i].frame_count();
        p.doc.panels[i].layers[0].frames = vec![Some(cel); n];
        p.modifier_textes(i, t.to_string(), String::new(), String::new()).unwrap();
    }
    crate::project::save(&p.doc, &manifest, &mut Default::default()).unwrap();
}

#[test]
fn a_project_is_saved_reopened_and_exported() {
    let dir = std::env::temp_dir().join(format!("pelure-test-noyau-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("Bus.pelure");
    let mut p = Projet::nouveau(320, 180, 24);
    assert!(p.enregistrer().is_err(), "pas encore de dossier");
    p.ajouter_panneau(0).unwrap();
    p.modifier_textes(0, "Le bus arrive".into(), String::new(), String::new()).unwrap();
    crate::project::save(&p.doc, &manifest, &mut Default::default()).unwrap();
    let (mut back, warnings) = Projet::ouvrir(&manifest).unwrap();
    assert!(warnings.is_empty());
    let r = back.resume();
    assert_eq!(r.nom, "Bus");
    assert_eq!(r.sequences[0].scenes.len(), 2, "deux plans");
    assert_eq!(r.sequences[0].scenes[0].panneaux[0].action, "Le bus arrive");
    back.modifier_textes(1, String::new(), "Bonjour".into(), String::new()).unwrap();
    back.enregistrer().unwrap();
    assert!(!back.resume().modifie);
    let (again, _) = Projet::ouvrir(&manifest).unwrap();
    assert_eq!(again.doc.panels[1].dialogue, "Bonjour");
    let pdf = dir.join("Bus.pdf");
    again.exporter_pdf(&pdf, "Bus").unwrap();
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_companion_never_writes_over_a_shared_film() {
    let dir = std::env::temp_dir().join(format!("pelure-test-noyau-partage-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let structure = dir.join("Film.pelure");
    let avant = br#"{"version": 1, "partage": {}}"#;
    std::fs::write(&structure, avant).unwrap();
    let mut p = Projet::nouveau(320, 180, 24);
    p.chemin = Some(structure.clone());
    assert!(p.enregistrer().is_err(), "un film partagé ne s'enregistre que par SHOT");
    assert_eq!(std::fs::read(&structure).unwrap(), avant.to_vec(), "la structure est intacte");
    assert!(!structure.with_extension("shot").exists());
    std::fs::remove_dir_all(&dir).ok();
}
