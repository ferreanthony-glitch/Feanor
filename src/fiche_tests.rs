//! Fiche technique (`fiche.rs`, `ficheformats.rs`, `zipwrite.rs`, `fichejob.rs`, `ficheui.rs`) : les colonnes
//! et leur contenu (numéros, versions, time codes, durées, cadrage, mouvement de caméra, transitions, sons,
//! commentaires, envois), le CSV (BOM, séparateurs, guillemets, formules), le texte du presse-papiers, le
//! classeur XLSX relu (archive, XML, cellules, styles, volet figé, images), la tâche (atomique, annulable,
//! fichier verrouillé) et la fenêtre. Aucun de ces fichiers n'a été ouvert dans Excel, LibreOffice ou Google
//! Sheets ici : voir `docs/INTERFACE.md`, partie 26.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;

use crate::app::App;
use crate::audio::Pcm;
use crate::camera::{Camera, CameraKey};
use crate::document::Document;
use crate::envois::Etat;
use crate::fiche::{self, Cellule, Colonne, Entree, Fiche, Reglage, Sens};
use crate::fichejob::{self, FicheJob};
use crate::ficheformats::{self, Format, Separateur};
use crate::relectureui::Etendue;
use crate::transitions::Kind;
use crate::xml;
use crate::zipread::Zip;
use crate::zipwrite::ZipWriter;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-fiche-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Quatre plans de 24, 12, 36 et 24 images, les trois premiers dans la séquence 1, le dernier dans la 2.
/// Numérotation verrouillée.
fn film() -> Document {
    let mut doc = Document::new(64, 36, 24);
    doc.panels[0].set_length(24);
    let i = doc.add_scene_after(0);
    doc.panels[i].set_length(12);
    let i = doc.add_scene_after(1);
    doc.panels[i].set_length(36);
    let i = doc.add_sequence_after(2);
    doc.panels[i].set_length(24);
    doc.lock_numbering();
    doc
}

fn tous(doc: &Document) -> Vec<usize> {
    (0..doc.panels.len()).collect()
}

fn fiche_de(doc: &Document, plans: &[usize], colonnes: &[Colonne], par_image: bool) -> Fiche {
    fiche::build(doc, &Entree { plans, colonnes, par_image, start_tc: 86_400, etats: None, envoi: None }).expect("fiche")
}

fn texte(f: &Fiche, ligne: usize, c: Colonne) -> String {
    let k = f.colonnes.iter().position(|x| *x == c).unwrap_or_else(|| panic!("colonne {c:?} absente"));
    match &f.lignes[ligne].cellules[k] {
        Cellule::Vide => String::new(),
        Cellule::Texte(t) => t.clone(),
        Cellule::Entier(n) => n.to_string(),
        Cellule::Decimal(x) => x.to_string(),
    }
}

fn cellule(f: &Fiche, ligne: usize, c: Colonne) -> Cellule {
    let k = f.colonnes.iter().position(|x| *x == c).unwrap();
    f.lignes[ligne].cellules[k].clone()
}

fn cam(x: f32, y: f32) -> Camera {
    Camera { x, y, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 }
}

const CADRE: [usize; 2] = [1920, 1080];

fn label(cams: &[Camera]) -> String {
    fiche::mouvement(cams, CADRE).label()
}

// ------------------------------------------------------------------------------------ colonnes

#[test]
fn every_column_has_a_stable_id_a_title_and_a_help() {
    let mut ids = std::collections::HashSet::new();
    let mut titres = std::collections::HashSet::new();
    for c in Colonne::TOUTES {
        assert!(ids.insert(c.id()), "{c:?} : identifiant en double");
        assert!(titres.insert(c.titre()), "{c:?} : titre en double");
        assert_eq!(Colonne::depuis_id(c.id()), Some(c));
        assert!(!c.aide().is_empty() && c.largeur() > 5.0);
    }
    assert_eq!(Colonne::TOUTES.len(), 30);
    assert_eq!(Colonne::depuis_id("n_importe_quoi"), None);
    // Les colonnes d'origine : sans doublon, dans l'ordre de la fenêtre.
    let d = fiche::colonnes_par_defaut();
    assert!(d.contains(&Colonne::Numero) && d.contains(&Colonne::Mouvement) && d.contains(&Colonne::Vignette) && !d.contains(&Colonne::Reference));
}

#[test]
fn saved_ids_keep_their_order_and_unknown_or_repeated_ones_are_ignored() {
    let ids: Vec<String> = ["notes", "numero", "inconnu", "notes", "duree_images"].iter().map(|s| s.to_string()).collect();
    assert_eq!(fiche::colonnes_depuis_ids(&ids), vec![Colonne::Notes, Colonne::Numero, Colonne::DureeImages]);
    assert!(fiche::colonnes_depuis_ids(&[]).is_empty());
}

#[test]
fn the_settings_survive_a_round_trip_through_the_settings_file() {
    let mut s = crate::settings::Settings::default();
    s.fiche.colonnes = vec!["notes".into(), "numero".into()];
    s.fiche.format = Format::Csv;
    s.fiche.separateur = Separateur::Virgule;
    s.fiche.par_image = true;
    s.fiche.start_tc = "10:00:00:00".into();
    s.fiche.dossier = Some(PathBuf::from("C:/Fiches"));
    let text = toml::to_string(&s).expect("écriture");
    let back: crate::settings::Settings = toml::from_str(&text).expect("lecture");
    assert_eq!(back.fiche, s.fiche);
    // Des réglages d'avant (sans la section) donnent les réglages d'origine.
    let vide: crate::settings::Settings = toml::from_str("").expect("lecture");
    assert_eq!(vide.fiche, fiche::Reglages::default());
    assert_eq!(vide.fiche.format, Format::Xlsx);
    assert_eq!(vide.fiche.separateur, Separateur::PointVirgule);
    assert_eq!(vide.fiche.start_tc, "01:00:00:00");
}

// ------------------------------------------------------------------------------------ numéros et versions

#[test]
fn numbers_include_inserted_plans_and_the_version_only_when_there_are_several() {
    let mut doc = film();
    // Un plan inséré entre le 2 et le 3 en numérotation verrouillée : 2A.
    let i = doc.add_scene_after(1);
    assert_eq!(doc.plan_code(doc.panels[i].scene), "2A");
    let cols = [Colonne::Sequence, Colonne::Numero, Colonne::Reference, Colonne::Version];
    let f = fiche_de(&doc, &tous(&doc), &cols, false);
    let numeros: Vec<String> = (0..f.lignes.len()).map(|k| texte(&f, k, Colonne::Numero)).collect();
    assert_eq!(numeros, ["1", "2", "2A", "3", "4"]);
    assert_eq!(texte(&f, 2, Colonne::Reference), "SQ01_PL002A");
    assert_eq!(texte(&f, 4, Colonne::Sequence), "2");
    assert!((0..5).all(|k| texte(&f, k, Colonne::Version).is_empty()), "une seule version : pas de V");
    // Une deuxième version : « V2 » et la référence la porte (comme le nom du clip du montage).
    doc.nouvelle_version(0, 1_760_000_000).expect("V2");
    let f = fiche_de(&doc, &tous(&doc), &cols, false);
    assert_eq!(texte(&f, 0, Colonne::Version), "V2");
    assert_eq!(texte(&f, 0, Colonne::Reference), "SQ01_PL001_V2");
    assert_eq!(texte(&f, 1, Colonne::Version), "");
    assert_eq!(texte(&f, 1, Colonne::Reference), "SQ01_PL002");
}

#[test]
fn a_custom_name_sequence_name_and_the_plan_colour_come_out() {
    let mut doc = film();
    let scene = doc.panels[1].scene;
    doc.scene_mut(scene).unwrap().name = "  Arrivée du train  ".into();
    let seq = doc.sequence_of(0);
    doc.sequences.iter_mut().find(|s| s.id == seq).unwrap().name = "Prologue".into();
    doc.panels[2].color = Some(11);
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Sequence, Colonne::Nom, Colonne::Couleur], false);
    assert_eq!(texte(&f, 0, Colonne::Sequence), "Prologue");
    assert_eq!(texte(&f, 0, Colonne::Nom), "");
    assert_eq!(texte(&f, 1, Colonne::Nom), "Arrivée du train");
    assert_eq!(texte(&f, 2, Colonne::Couleur), "Orange");
    assert_eq!(texte(&f, 3, Colonne::Couleur), "");
}

// ------------------------------------------------------------------------------------ temps

#[test]
fn durations_and_time_codes_follow_the_whole_film_even_for_a_few_plans() {
    let doc = film();
    let cols = [Colonne::DureeImages, Colonne::DureeSecondes, Colonne::DureeTc, Colonne::TcEntree, Colonne::TcSortie];
    let f = fiche_de(&doc, &tous(&doc), &cols, false);
    assert_eq!(cellule(&f, 0, Colonne::DureeImages), Cellule::Entier(24));
    assert_eq!(cellule(&f, 1, Colonne::DureeImages), Cellule::Entier(12));
    assert_eq!(cellule(&f, 0, Colonne::DureeSecondes), Cellule::Decimal(1.0));
    assert_eq!(cellule(&f, 1, Colonne::DureeSecondes), Cellule::Decimal(0.5));
    assert_eq!(texte(&f, 2, Colonne::DureeTc), "00:00:01:12");
    let entrees: Vec<String> = (0..4).map(|k| texte(&f, k, Colonne::TcEntree)).collect();
    assert_eq!(entrees, ["01:00:00:00", "01:00:01:00", "01:00:01:12", "01:00:03:00"]);
    assert_eq!(texte(&f, 3, Colonne::TcSortie), "01:00:04:00");
    assert_eq!(texte(&f, 0, Colonne::TcSortie), texte(&f, 1, Colonne::TcEntree), "la sortie d'un plan est l'entrée du suivant");
    // Seulement les deux derniers plans : leurs time codes sont ceux du film entier.
    let f = fiche_de(&doc, &[2, 3], &cols, false);
    assert_eq!(f.lignes.len(), 2);
    assert_eq!(texte(&f, 0, Colonne::TcEntree), "01:00:01:12");
    // Une autre heure de départ.
    let f = fiche::build(&doc, &Entree { plans: &[0], colonnes: &cols, par_image: false, start_tc: editorial_tc("00:59:58:12"), etats: None, envoi: None }).unwrap();
    assert_eq!(texte(&f, 0, Colonne::TcEntree), "00:59:58:12");
    assert_eq!(texte(&f, 0, Colonne::TcSortie), "00:59:59:12");
}

fn editorial_tc(s: &str) -> i64 {
    crate::editorial::parse_timecode(s, 24).expect("time code")
}

#[test]
fn a_fractional_duration_is_rounded_to_a_thousandth() {
    let mut doc = film();
    doc.panels[1].set_length(13);
    let f = fiche_de(&doc, &[1], &[Colonne::DureeSecondes], false);
    assert_eq!(cellule(&f, 0, Colonne::DureeSecondes), Cellule::Decimal(0.542));
}

// ------------------------------------------------------------------------------------ cadrage

#[test]
fn the_framing_is_the_lens_preset_name_or_the_focal_length() {
    let mut doc = film();
    let mut c = doc.default_camera();
    c.zoom = 85.0 / 35.0;
    doc.panels[0].camera = vec![CameraKey::new(0, c)];
    let mut d = doc.default_camera();
    d.zoom = 1.5;
    doc.panels[1].camera = vec![CameraKey::new(0, d)];
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Cadrage], false);
    assert_eq!(texte(&f, 0, Colonne::Cadrage), "Portrait · 85 mm");
    assert_eq!(texte(&f, 1, Colonne::Cadrage), "52,5 mm");
    assert_eq!(texte(&f, 2, Colonne::Cadrage), "Normale · 35 mm", "sans clé : la caméra au centre, 35 mm");
}

// ------------------------------------------------------------------------------------ mouvement

#[test]
fn nothing_moves_a_camera_with_no_key_one_key_or_equal_keys() {
    assert_eq!(label(&[]), "fixe");
    assert_eq!(label(&[cam(960.0, 540.0)]), "fixe");
    assert_eq!(label(&[cam(960.0, 540.0), cam(960.0, 540.0)]), "fixe");
    assert!(fiche::mouvement(&[cam(1.0, 1.0)], CADRE).est_fixe());
    assert_eq!(fiche::mouvement_detail(&[cam(960.0, 540.0), cam(960.0, 540.0)], CADRE), "");
}

#[test]
fn a_slide_across_the_sheet_is_a_travelling_with_its_direction() {
    assert_eq!(label(&[cam(960.0, 540.0), cam(1200.0, 540.0)]), "travelling à droite");
    assert_eq!(label(&[cam(960.0, 540.0), cam(700.0, 540.0)]), "travelling à gauche");
    assert_eq!(label(&[cam(960.0, 540.0), cam(960.0, 300.0)]), "travelling vers le haut");
    assert_eq!(label(&[cam(960.0, 540.0), cam(960.0, 800.0)]), "travelling vers le bas");
    assert_eq!(label(&[cam(960.0, 540.0), cam(1200.0, 800.0)]), "travelling à droite et vers le bas");
}

#[test]
fn the_rotating_camera_is_a_pan_or_a_tilt() {
    let a = cam(960.0, 540.0);
    assert_eq!(label(&[a, Camera { pan: 20.0, ..a }]), "panoramique à droite");
    assert_eq!(label(&[a, Camera { pan: -20.0, ..a }]), "panoramique à gauche");
    assert_eq!(label(&[a, Camera { tilt: 8.0, ..a }]), "bascule vers le haut");
    assert_eq!(label(&[a, Camera { tilt: -8.0, ..a }]), "bascule vers le bas");
    assert_eq!(label(&[a, Camera { angle: 15.0, ..a }]), "rotation horaire");
    assert_eq!(label(&[a, Camera { angle: -15.0, ..a }]), "rotation antihoraire");
}

#[test]
fn the_lens_is_a_zoom_and_the_advance_a_travelling_in_or_out() {
    let a = cam(960.0, 540.0);
    assert_eq!(label(&[a, Camera { zoom: 1.5, ..a }]), "zoom avant");
    assert_eq!(label(&[a, Camera { zoom: 0.5, ..a }]), "zoom arrière");
    assert_eq!(label(&[a, Camera { dolly: 400.0, ..a }]), "travelling avant");
    assert_eq!(label(&[a, Camera { dolly: -400.0, ..a }]), "travelling arrière");
}

#[test]
fn several_movements_are_joined_in_a_fixed_order() {
    let a = cam(960.0, 540.0);
    let b = Camera { x: 1300.0, zoom: 1.4, pan: 12.0, tilt: -6.0, angle: 5.0, dolly: 300.0, ..a };
    assert_eq!(label(&[a, b]), "travelling à droite + travelling avant + panoramique à droite + bascule vers le bas + zoom avant + rotation horaire");
    let detail = fiche::mouvement_detail(&[a, b], CADRE);
    assert!(detail.contains("X : 960 → 1300 px"), "{detail}");
    assert!(detail.contains("Focale : 35 → 49 mm"), "{detail}");
    assert!(detail.contains("Panoramique : 0 → 12°") && detail.contains("Bascule : 0 → -6°") && detail.contains("Rotation : 0 → 5°") && detail.contains("Avancée : 0 → 300 px"), "{detail}");
    assert!(!detail.contains("Y :"), "ce qui ne bouge pas n'est pas dit : {detail}");
    // Le zoom et un panoramique ensemble.
    assert_eq!(label(&[a, Camera { zoom: 2.0, pan: 10.0, ..a }]), "panoramique à droite + zoom avant");
}

#[test]
fn the_thresholds_separate_a_real_move_from_a_wobble() {
    let a = cam(960.0, 540.0);
    // Sous 1 % de la largeur (19 px), 2 % d'avancée, 2 % de focale, 0,5° : fixe.
    assert_eq!(label(&[a, cam(960.0 + 18.0, 540.0)]), "fixe");
    assert_eq!(label(&[a, cam(960.0 + 20.0, 540.0)]), "travelling à droite");
    assert_eq!(label(&[a, cam(960.0, 540.0 + 10.0)]), "fixe", "1 % de la hauteur, c'est 10,8 px");
    assert_eq!(label(&[a, cam(960.0, 540.0 + 11.0)]), "travelling vers le bas");
    assert_eq!(label(&[a, Camera { zoom: 1.01, ..a }]), "fixe");
    assert_eq!(label(&[a, Camera { zoom: 1.03, ..a }]), "zoom avant");
    assert_eq!(label(&[a, Camera { dolly: 30.0, ..a }]), "fixe");
    assert_eq!(label(&[a, Camera { dolly: 40.0, ..a }]), "travelling avant");
    assert_eq!(label(&[a, Camera { pan: 0.4, tilt: 0.4, angle: 0.4, ..a }]), "fixe");
    assert_eq!(label(&[a, Camera { pan: 0.6, ..a }]), "panoramique à droite");
}

#[test]
fn a_move_that_comes_back_is_a_round_trip_not_a_still_shot() {
    let a = cam(960.0, 540.0);
    let loin = cam(1400.0, 540.0);
    assert_eq!(label(&[a, loin, a]), "travelling latéral aller-retour");
    assert_eq!(label(&[a, Camera { zoom: 2.0, ..a }, a]), "zoom avant-arrière");
    // Aller loin puis revenir à mi-chemin : le sens net est celui de l'arrivée.
    assert_eq!(label(&[a, cam(1500.0, 540.0), cam(1300.0, 540.0)]), "travelling à droite");
}

#[test]
fn the_movement_comes_from_the_camera_keys_of_the_plan_and_of_each_storyboard_image() {
    let mut doc = film();
    let a = doc.default_camera();
    // Plan 1 : un panoramique en image 0 -> 23 ; plan 2 : une seule clé ; plan 3 : sans clé.
    doc.panels[0].camera = vec![CameraKey::new(0, a), CameraKey::new(23, Camera { pan: 15.0, ..a })];
    doc.panels[1].camera = vec![CameraKey::new(3, Camera { zoom: 2.0, ..a })];
    // Plan 4 : mouvement dans sa seconde moitié seulement, avec une marque d'image à la 12e.
    doc.panels[3].camera = vec![CameraKey::new(0, a), CameraKey::new(11, a), CameraKey::new(23, Camera { x: a.x + 20.0, ..a })];
    doc.toggle_mark(3, 12).expect("marque");
    let cols = [Colonne::Mouvement, Colonne::MouvementDetail, Colonne::Image];
    let f = fiche_de(&doc, &tous(&doc), &cols, false);
    assert_eq!(texte(&f, 0, Colonne::Mouvement), "panoramique à droite");
    assert_eq!(texte(&f, 0, Colonne::MouvementDetail), "Panoramique : 0 → 15°");
    assert_eq!(texte(&f, 1, Colonne::Mouvement), "fixe", "une seule clé : la caméra ne bouge pas");
    assert_eq!(texte(&f, 2, Colonne::Mouvement), "fixe", "aucune clé");
    assert_eq!(texte(&f, 3, Colonne::Mouvement), "travelling à droite");
    // Une ligne par image de storyboard : la première moitié du plan 4 est fixe, la seconde bouge.
    let f = fiche_de(&doc, &[3], &cols, true);
    assert_eq!(f.lignes.len(), 2);
    assert_eq!(texte(&f, 0, Colonne::Mouvement), "fixe");
    assert_eq!(texte(&f, 1, Colonne::Mouvement), "travelling à droite");
    assert_eq!((texte(&f, 0, Colonne::Image), texte(&f, 1, Colonne::Image)), ("1 / 2".to_string(), "2 / 2".to_string()));
}

#[test]
fn the_reglages_of_the_table_are_named_the_way_the_help_says() {
    // La classification ne rend que les sept réglages décrits.
    let a = cam(960.0, 540.0);
    let m = fiche::mouvement(&[a, Camera { x: 1500.0, y: 100.0, zoom: 3.0, angle: 20.0, dolly: 500.0, tilt: 9.0, pan: 9.0 }], CADRE);
    let reglages: Vec<Reglage> = m.parties.iter().map(|p| p.0).collect();
    assert_eq!(reglages, [Reglage::X, Reglage::Y, Reglage::Avancee, Reglage::Panoramique, Reglage::Bascule, Reglage::Zoom, Reglage::Rotation]);
    assert!(m.parties.iter().all(|p| p.1 == Sens::Plus || p.0 == Reglage::Y));
}

// ------------------------------------------------------------------------------------ images de storyboard

#[test]
fn one_row_per_storyboard_image_carries_its_own_texts_and_time_codes() {
    let mut doc = film();
    doc.panels[2].action = "Le train entre en gare.".into();
    doc.panels[2].dialogue = "— Quai deux !".into();
    doc.toggle_mark(2, 10).expect("marque 1");
    doc.toggle_mark(2, 25).expect("marque 2");
    doc.panels[2].marks[0].action = "Les portes s'ouvrent.".into();
    doc.panels[2].marks[1].notes = "Plan large".into();
    let cols = [Colonne::Numero, Colonne::Image, Colonne::ImagesStoryboard, Colonne::DureeImages, Colonne::TcEntree, Colonne::TcSortie, Colonne::Action, Colonne::Dialogue, Colonne::Notes, Colonne::Vignette];
    let f = fiche_de(&doc, &[2], &cols, true);
    assert_eq!(f.lignes.len(), 3);
    let images: Vec<String> = (0..3).map(|k| texte(&f, k, Colonne::Image)).collect();
    assert_eq!(images, ["1 / 3", "2 / 3", "3 / 3"]);
    let durees: Vec<String> = (0..3).map(|k| texte(&f, k, Colonne::DureeImages)).collect();
    assert_eq!(durees, ["10", "15", "11"]);
    assert_eq!(texte(&f, 0, Colonne::ImagesStoryboard), "3");
    assert_eq!(texte(&f, 1, Colonne::TcEntree), "01:00:01:22");
    assert_eq!(texte(&f, 2, Colonne::TcEntree), "01:00:02:13");
    assert_eq!(texte(&f, 2, Colonne::TcSortie), "01:00:03:00");
    assert_eq!(texte(&f, 0, Colonne::Action), "Le train entre en gare.");
    assert_eq!(texte(&f, 0, Colonne::Dialogue), "— Quai deux !");
    assert_eq!(texte(&f, 1, Colonne::Action), "Les portes s'ouvrent.");
    assert_eq!(texte(&f, 2, Colonne::Notes), "Plan large");
    assert_eq!(texte(&f, 2, Colonne::Action), "", "une marque a ses propres textes");
    // Une vignette par image : la première image de chacune ; des noms qui ne se marchent pas dessus.
    let images = f.images_vignettes();
    assert_eq!(images, vec![(2, 0), (2, 10), (2, 25)]);
    assert_eq!(f.fichiers_vignettes(), ["SQ01_PL003.png", "SQ01_PL003_img02.png", "SQ01_PL003_img03.png"]);
    assert_eq!(texte(&f, 1, Colonne::Vignette), "vignettes/SQ01_PL003_img02.png");
    // Une ligne par plan : le nombre d'images de storyboard est dit, une seule vignette.
    let f = fiche_de(&doc, &[2], &cols, false);
    assert_eq!(f.lignes.len(), 1);
    assert_eq!(texte(&f, 0, Colonne::ImagesStoryboard), "3");
    assert_eq!(texte(&f, 0, Colonne::Image), "");
    assert_eq!(f.images_vignettes(), vec![(2, 0)]);
}

// ------------------------------------------------------------------------------------ le reste du plan

#[test]
fn transitions_come_from_the_cuts_on_each_side_of_the_plan() {
    let mut doc = film();
    let id = doc.set_transition(1, Kind::Dissolve).expect("fondu");
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().frames = 8;
    let id = doc.set_transition(0, Kind::Black).expect("noir");
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().frames = 6;
    let id = doc.set_transition(4, Kind::White).expect("blanc");
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().frames = 10;
    let cols = [Colonne::TransitionEntree, Colonne::TransitionSortie];
    let f = fiche_de(&doc, &tous(&doc), &cols, false);
    assert_eq!(texte(&f, 0, Colonne::TransitionEntree), "Fondu au noir (6 i)");
    assert_eq!(texte(&f, 0, Colonne::TransitionSortie), "Fondu enchaîné (8 i)");
    assert_eq!(texte(&f, 1, Colonne::TransitionEntree), "Fondu enchaîné (8 i)");
    assert_eq!(texte(&f, 1, Colonne::TransitionSortie), "");
    assert_eq!(texte(&f, 3, Colonne::TransitionSortie), "Fondu au blanc (10 i)");
    // Une ligne par image de storyboard : l'entrée sur la première, la sortie sur la dernière.
    doc.toggle_mark(0, 10).expect("marque");
    let f = fiche_de(&doc, &[0], &cols, true);
    assert_eq!((texte(&f, 0, Colonne::TransitionEntree), texte(&f, 0, Colonne::TransitionSortie)), ("Fondu au noir (6 i)".to_string(), String::new()));
    assert_eq!((texte(&f, 1, Colonne::TransitionEntree), texte(&f, 1, Colonne::TransitionSortie)), (String::new(), "Fondu enchaîné (8 i)".to_string()));
}
#[test]
fn sounds_comments_and_effects_details() {
    let mut doc = film();
    let voix = doc.add_sound("voix.wav".into(), "wav".into(), Arc::new(Vec::new()), Arc::new(Pcm { data: vec![100; 48_000 * 2 * 4], peaks: Vec::new() }));
    doc.add_audio_track();
    doc.add_clip(0, voix, 20, 0, 10).expect("voix");
    let plan = doc.panels[1].id;
    for (n, (t, auteur, fini, image)) in [("À refaire", "Anthony", false, 3usize), ("Fait", "Léa", true, 4), ("Le décor est trop vide", "", false, 99)].iter().enumerate() {
        doc.comments.push(crate::commentaires::Comment { id: n as u64 + 1, panel: plan, frame: *image, text: t.to_string(), author: auteur.to_string(), color: [60, 200, 215], created: 0, done: *fini, note: None, annotation: None, follow: Default::default() });
    }
    doc.panels[0].layers.push(crate::document::Layer::new("Fond", crate::document::LayerKind::Vector));
    for _ in 0..2 {
        doc.panels[0].layers[0].fx.push(crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur));
    }
    doc.panels[0].layers[1].fx.push(crate::fx::Effect::new(crate::fx::EffectKind::Glow));
    let mut eteint = crate::fx::Effect::new(crate::fx::EffectKind::Invert);
    eteint.enabled = false;
    doc.panels[0].layers[1].fx.push(eteint);
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Sons, Colonne::Commentaires, Colonne::CommentairesTexte, Colonne::Effets, Colonne::Calques], false);
    // La voix (images 20 à 29) recouvre la fin du plan 1 (0 à 23) et le début du plan 2 (24 à 35), pas les autres.
    assert_eq!(texte(&f, 0, Colonne::Sons), "voix.wav");
    assert_eq!(texte(&f, 1, Colonne::Sons), "voix.wav");
    assert_eq!(texte(&f, 2, Colonne::Sons), "");
    // Deux commentaires ouverts sur le plan 2 (le résolu ne compte pas) ; leurs textes, avec l'auteur.
    assert_eq!(cellule(&f, 1, Colonne::Commentaires), Cellule::Entier(2));
    assert_eq!(cellule(&f, 0, Colonne::Commentaires), Cellule::Entier(0));
    assert_eq!(texte(&f, 1, Colonne::CommentairesTexte), "Anthony : À refaire\nLe décor est trop vide");
    // Les effets allumés, comptés quand ils se répètent ; un effet éteint n'y est pas.
    assert_eq!(texte(&f, 0, Colonne::Effets), "Flou gaussien × 2; Lueur");
    assert_eq!(texte(&f, 1, Colonne::Effets), "");
    assert_eq!(cellule(&f, 0, Colonne::Calques), Cellule::Entier(2));
    assert_eq!(cellule(&f, 1, Colonne::Calques), Cellule::Entier(doc.panels[1].layers.len() as i64));
}

#[test]
fn comments_of_a_storyboard_image_are_those_that_fall_on_it() {
    let mut doc = film();
    doc.toggle_mark(2, 10).expect("marque");
    let plan = doc.panels[2].id;
    for (n, image) in [2usize, 12, 99].iter().enumerate() {
        doc.comments.push(crate::commentaires::Comment { id: n as u64 + 1, panel: plan, frame: *image, text: format!("c{n}"), author: String::new(), color: [1, 2, 3], created: 0, done: false, note: None, annotation: None, follow: Default::default() });
    }
    let f = fiche_de(&doc, &[2], &[Colonne::Commentaires], true);
    // Image 1 : le commentaire de l'image 2 ; image 2 : celui de l'image 12 et, sur la dernière image, celui qui dépasse le plan.
    assert_eq!((cellule(&f, 0, Colonne::Commentaires), cellule(&f, 1, Colonne::Commentaires)), (Cellule::Entier(1), Cellule::Entier(2)));
}

#[test]
fn modified_since_an_envoi_needs_the_envoi_and_its_survey() {
    let doc = film();
    let mut etats = HashMap::new();
    etats.insert(doc.panels[1].id, Etat::Modifie);
    etats.insert(doc.panels[3].id, Etat::Nouveau);
    let cols = [Colonne::Modifie, Colonne::Envoi];
    let f = fiche::build(&doc, &Entree { plans: &[0, 1, 2, 3], colonnes: &cols, par_image: false, start_tc: 0, etats: Some(&etats), envoi: Some("v2 au réal") }).unwrap();
    let modifies: Vec<String> = (0..4).map(|k| texte(&f, k, Colonne::Modifie)).collect();
    assert_eq!(modifies, ["", "Modifié", "", "Ajouté"]);
    assert_eq!(texte(&f, 0, Colonne::Envoi), "v2 au réal");
    assert!(f.avertissements.is_empty());
    // Sans envoi dans le projet : colonnes vides, rien à dire.
    let f = fiche::build(&doc, &Entree { plans: &[0], colonnes: &cols, par_image: false, start_tc: 0, etats: None, envoi: None }).unwrap();
    assert_eq!((texte(&f, 0, Colonne::Modifie), texte(&f, 0, Colonne::Envoi)), (String::new(), String::new()));
    assert!(f.avertissements.is_empty());
    // L'envoi existe mais son relevé n'est pas prêt : on le dit.
    let f = fiche::build(&doc, &Entree { plans: &[0], colonnes: &cols, par_image: false, start_tc: 0, etats: None, envoi: Some("v2") }).unwrap();
    assert_eq!(f.avertissements.len(), 1, "{:?}", f.avertissements);
}

#[test]
fn nothing_to_write_says_so() {
    let doc = film();
    assert!(fiche::build(&doc, &Entree { plans: &[], colonnes: &[Colonne::Numero], par_image: false, start_tc: 0, etats: None, envoi: None }).is_err());
    assert!(fiche::build(&doc, &Entree { plans: &[99], colonnes: &[Colonne::Numero], par_image: false, start_tc: 0, etats: None, envoi: None }).is_err(), "un plan qui n'existe pas");
    assert!(fiche::build(&doc, &Entree { plans: &[0], colonnes: &[], par_image: false, start_tc: 0, etats: None, envoi: None }).is_err(), "aucune colonne");
    // L'ordre et les doublons de la liste des plans n'importent pas : la fiche suit le film.
    let f = fiche_de(&doc, &[3, 1, 1, 0], &[Colonne::Numero], false);
    let n: Vec<String> = (0..f.lignes.len()).map(|k| texte(&f, k, Colonne::Numero)).collect();
    assert_eq!(n, ["1", "2", "4"]);
}

// ------------------------------------------------------------------------------------ CSV

fn petite_fiche(valeurs: &[Cellule]) -> Fiche {
    let colonnes = vec![Colonne::Nom, Colonne::DureeImages, Colonne::DureeSecondes, Colonne::Notes];
    let mut cellules = valeurs.to_vec();
    cellules.resize(4, Cellule::Vide);
    Fiche { colonnes, lignes: vec![fiche::Ligne { cellules, vignette: None }], avertissements: Vec::new() }
}

#[test]
fn csv_has_a_bom_crlf_a_header_and_the_chosen_separator() {
    let f = petite_fiche(&[Cellule::Texte("Gare".into()), Cellule::Entier(24), Cellule::Decimal(1.5), Cellule::Texte("ok".into())]);
    let pv = ficheformats::csv(&f, Separateur::PointVirgule);
    assert_eq!(&pv[..3], &[0xEF, 0xBB, 0xBF], "BOM UTF-8");
    let texte = String::from_utf8(pv[3..].to_vec()).unwrap();
    assert_eq!(texte, "Nom du plan;Durée (images);Durée (s);Notes\r\nGare;24;1,5;ok\r\n");
    let v = ficheformats::csv(&f, Separateur::Virgule);
    assert_eq!(String::from_utf8(v[3..].to_vec()).unwrap(), "Nom du plan,Durée (images),Durée (s),Notes\r\nGare,24,1.5,ok\r\n");
}

#[test]
fn csv_quotes_what_needs_it_and_keeps_long_texts_whole() {
    let long = "mot ".repeat(20_000);
    let f = petite_fiche(&[Cellule::Texte("a;b".into()), Cellule::Vide, Cellule::Vide, Cellule::Texte(format!("ligne 1\nligne \"2\", fin\r\n{long}"))]);
    let pv = String::from_utf8(ficheformats::csv(&f, Separateur::PointVirgule)[3..].to_vec()).unwrap();
    let ligne = pv.split("\r\n").nth(1).unwrap_or("");
    assert!(pv.contains("\"a;b\";;;\"ligne 1\nligne \"\"2\"\", fin\n"), "{}", &pv[..200.min(pv.len())]);
    assert!(pv.contains(&long), "le texte long est entier");
    let _ = ligne;
    // Avec la virgule, le point-virgule n'a rien de spécial.
    let v = String::from_utf8(ficheformats::csv(&f, Separateur::Virgule)[3..].to_vec()).unwrap();
    assert!(v.contains("\na;b,,,\"ligne 1"), "{}", &v[..200.min(v.len())]);
    // Un lecteur CSV minimal retrouve exactement les cellules.
    let lignes = lire_csv(&pv, ';');
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[1][0], "a;b");
    assert_eq!(lignes[1][3], format!("ligne 1\nligne \"2\", fin\n{long}"));
}

/// Un lecteur CSV (guillemets, séparateur, retours à la ligne) pour relire ce qu'on écrit.
fn lire_csv(t: &str, sep: char) -> Vec<Vec<String>> {
    let mut lignes = Vec::new();
    let (mut ligne, mut champ, mut guillemets) = (Vec::new(), String::new(), false);
    let mut it = t.chars().peekable();
    while let Some(c) = it.next() {
        match (c, guillemets) {
            ('"', true) if it.peek() == Some(&'"') => {
                champ.push('"');
                it.next();
            }
            ('"', _) => guillemets = !guillemets,
            (c, false) if c == sep => ligne.push(std::mem::take(&mut champ)),
            ('\r', false) => {}
            ('\n', false) => {
                ligne.push(std::mem::take(&mut champ));
                lignes.push(std::mem::take(&mut ligne));
            }
            (c, _) => champ.push(c),
        }
    }
    lignes
}

#[test]
fn csv_neutralises_cells_that_a_spreadsheet_would_read_as_a_formula() {
    for dangereux in ["=1+1", "+33 6 12", "-1", "@SOMME(A1)", "=HYPERLINK(\"http://x\";\"a\")", "\t=1", "\r=1"] {
        assert_eq!(ficheformats::neutralise(dangereux).chars().next(), Some('\''), "{dangereux:?}");
    }
    for sur in ["", "Salut", "— Salut", "a=b", "1+1", " =1", "12A", "'déjà"] {
        assert_eq!(ficheformats::neutralise(sur), sur, "{sur:?}");
    }
    let f = petite_fiche(&[Cellule::Texte("=cmd|' /C calc'!A0".into()), Cellule::Entier(-3), Cellule::Decimal(-1.5), Cellule::Texte("-\n@x".into())]);
    let csv = String::from_utf8(ficheformats::csv(&f, Separateur::PointVirgule)[3..].to_vec()).unwrap();
    let lignes = lire_csv(&csv, ';');
    assert_eq!(lignes[1][0], "'=cmd|' /C calc'!A0");
    assert_eq!(lignes[1][1], "-3", "un nombre écrit par SHOT n'est pas du texte : il reste un nombre");
    assert_eq!(lignes[1][2], "-1,5");
    assert_eq!(lignes[1][3], "'-\n@x", "seul le début de la cellule compte");
}

#[test]
fn the_clipboard_text_is_tab_separated_quoted_and_neutralised() {
    let f = petite_fiche(&[Cellule::Texte("a\tb".into()), Cellule::Entier(12), Cellule::Decimal(2.25), Cellule::Texte("=1+1\nsuite \"x\"".into())]);
    let t = ficheformats::presse_papiers(&f);
    let lignes = lire_csv(&t, '\t');
    assert_eq!(lignes[0], ["Nom du plan", "Durée (images)", "Durée (s)", "Notes"]);
    assert_eq!(lignes[1], ["a\tb", "12", "2,25", "'=1+1\nsuite \"x\""]);
    assert!(!t.contains('\u{feff}'), "pas de BOM dans le presse-papiers");
    assert!(t.starts_with("Nom du plan\tDurée (images)"));
}

// ------------------------------------------------------------------------------------ zip

#[test]
fn the_zip_writer_is_read_back_by_the_zip_reader_whatever_the_method() {
    let mut z = ZipWriter::new();
    let gros = "abcde fghij ".repeat(5000);
    z.deflate("a/gros.xml", gros.as_bytes()).unwrap();
    z.store("b.png", &[1, 2, 3, 4]).unwrap();
    z.deflate("vide.txt", b"").unwrap();
    z.deflate("minuscule.txt", b"x").unwrap();
    let octets = z.finish().unwrap();
    let zip = Zip::open(&octets).expect("archive");
    let noms: Vec<&str> = zip.names().collect();
    assert_eq!(noms, ["a/gros.xml", "b.png", "vide.txt", "minuscule.txt"]);
    assert_eq!(zip.read("a/gros.xml").unwrap(), gros.as_bytes());
    assert_eq!(zip.read("b.png").unwrap(), [1, 2, 3, 4]);
    assert_eq!(zip.read("vide.txt").unwrap(), b"");
    assert_eq!(zip.read("minuscule.txt").unwrap(), b"x");
    assert!(octets.len() < gros.len() / 4, "le texte répétitif est compressé : {} octets", octets.len());
    // Le CRC-32 du répertoire est le bon (flate2 le calcule).
    let mut crc = flate2::Crc::new();
    crc.update(b"x");
    let attendu = crc.sum().to_le_bytes();
    assert!(octets.windows(4).any(|w| w == attendu), "CRC de « x »");
    // Rien ne change d'une écriture à l'autre.
    let mut z2 = ZipWriter::new();
    z2.deflate("a/gros.xml", gros.as_bytes()).unwrap();
    z2.store("b.png", &[1, 2, 3, 4]).unwrap();
    z2.deflate("vide.txt", b"").unwrap();
    z2.deflate("minuscule.txt", b"x").unwrap();
    assert_eq!(z2.finish().unwrap(), octets);
    // Les noms dangereux sont refusés.
    for mauvais in ["", "/etc/x", "..\\x", "a/../b", "c:\\x"] {
        assert!(ZipWriter::new().store(mauvais, b"1").is_err(), "{mauvais:?}");
    }
}

// ------------------------------------------------------------------------------------ XLSX

/// Un petit PNG de `w`×`h` (de la couleur `c`) avec le vrai encodeur.
fn png(w: u32, h: u32, c: u8) -> Vec<u8> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out).write_image(&vec![c; (w * h * 3) as usize], w, h, image::ExtendedColorType::Rgb8).unwrap();
    out
}

struct Classeur {
    zip_octets: Vec<u8>,
}

impl Classeur {
    fn zip(&self) -> Zip<'_> {
        Zip::open(&self.zip_octets).expect("archive")
    }

    fn xml(&self, nom: &str) -> xml::Element {
        let octets = self.zip().read(nom).unwrap_or_else(|| panic!("{nom} absent"));
        xml::parse(std::str::from_utf8(&octets).expect("UTF-8")).unwrap_or_else(|e| panic!("{nom} : {e}"))
    }

    /// Les chaînes partagées, dans l'ordre.
    fn chaines(&self) -> Vec<String> {
        self.xml("xl/sharedStrings.xml").children_named("si").map(|si| si.child("t").map_or(String::new(), |t| t.text.clone())).collect()
    }

    /// La feuille : pour chaque ligne, (numéro, hauteur, cellules (référence, style, type, valeur lue)).
    fn feuille(&self) -> Vec<(usize, Option<String>, Vec<(String, String, String, String)>)> {
        let chaines = self.chaines();
        let sheet = self.xml("xl/worksheets/sheet1.xml");
        let data = sheet.child("sheetData").expect("sheetData");
        data.children_named("row")
            .map(|r| {
                let cells = r
                    .children_named("c")
                    .map(|c| {
                        let t = c.attr("t").unwrap_or("").to_string();
                        let brut = c.child("v").map_or(String::new(), |v| v.text.clone());
                        let valeur = if t == "s" { chaines[brut.parse::<usize>().unwrap()].clone() } else { brut };
                        (c.attr("r").unwrap().to_string(), c.attr("s").unwrap_or("0").to_string(), t, valeur)
                    })
                    .collect();
                (r.attr("r").unwrap().parse().unwrap(), r.attr("ht").map(str::to_string), cells)
            })
            .collect()
    }
}

fn classeur(f: &Fiche, images: &[Vec<u8>]) -> (Classeur, Vec<String>) {
    let c = ficheformats::xlsx(f, images).expect("classeur");
    (Classeur { zip_octets: c.octets }, c.avertissements)
}

#[test]
fn the_workbook_has_every_part_declared_related_and_well_formed() {
    let doc = film();
    let f = fiche_de(&doc, &tous(&doc), &fiche::colonnes_par_defaut(), false);
    let images: Vec<Vec<u8>> = (0..f.images_vignettes().len()).map(|k| png(160, 90, k as u8)).collect();
    let (c, _) = classeur(&f, &images);
    let noms: Vec<String> = c.zip().names().map(str::to_string).collect();
    for attendu in ["[Content_Types].xml", "_rels/.rels", "xl/workbook.xml", "xl/_rels/workbook.xml.rels", "xl/styles.xml", "xl/sharedStrings.xml", "xl/worksheets/sheet1.xml", "xl/worksheets/_rels/sheet1.xml.rels", "xl/drawings/drawing1.xml", "xl/drawings/_rels/drawing1.xml.rels"] {
        assert!(noms.iter().any(|n| n == attendu), "{attendu} manque : {noms:?}");
        if attendu.ends_with(".xml") || attendu.ends_with(".rels") {
            c.xml(attendu);
        }
    }
    // Chaque partie XML est déclarée (par défaut « xml » / « rels » ou par un Override), les images en PNG.
    let types = c.xml("[Content_Types].xml");
    let defauts: Vec<&str> = types.children_named("Default").filter_map(|d| d.attr("Extension")).collect();
    assert!(defauts.contains(&"png") && defauts.contains(&"rels") && defauts.contains(&"xml"));
    let surcharges: Vec<&str> = types.children_named("Override").filter_map(|o| o.attr("PartName")).collect();
    for p in ["/xl/workbook.xml", "/xl/worksheets/sheet1.xml", "/xl/styles.xml", "/xl/sharedStrings.xml", "/xl/drawings/drawing1.xml"] {
        assert!(surcharges.contains(&p), "{p} non déclaré");
    }
    // La racine mène au classeur ; le classeur à la feuille, aux styles et aux chaînes.
    let racine = c.xml("_rels/.rels");
    assert_eq!(racine.children_named("Relationship").next().unwrap().attr("Target"), Some("xl/workbook.xml"));
    let rels = c.xml("xl/_rels/workbook.xml.rels");
    let cibles: Vec<&str> = rels.children_named("Relationship").filter_map(|r| r.attr("Target")).collect();
    assert_eq!(cibles, ["worksheets/sheet1.xml", "styles.xml", "sharedStrings.xml"]);
    for cible in cibles {
        assert!(noms.iter().any(|n| *n == format!("xl/{cible}")), "{cible}");
    }
    let wb = c.xml("xl/workbook.xml");
    assert_eq!(wb.path("sheets/sheet").unwrap().attr("name"), Some("Fiche technique"));
    // Le filtre automatique a son nom défini (c'est ce qu'écrit Excel).
    let nom = wb.path("definedNames/definedName").unwrap();
    assert_eq!(nom.attr("name"), Some("_xlnm._FilterDatabase"));
    assert!(nom.text.starts_with("'Fiche technique'!$A$1:$"), "{}", nom.text);
}

#[test]
fn the_sheet_has_a_bold_frozen_header_widths_a_filter_and_typed_cells() {
    let doc = film();
    let cols = [Colonne::Numero, Colonne::DureeImages, Colonne::DureeSecondes, Colonne::TcEntree, Colonne::Action];
    let mut doc = doc;
    doc.panels[0].action = "Ligne 1\nLigne 2".into();
    let f = fiche_de(&doc, &tous(&doc), &cols, false);
    let (c, avertissements) = classeur(&f, &[]);
    assert!(avertissements.is_empty());
    let feuille = c.xml("xl/worksheets/sheet1.xml");
    // Volet figé sur la première ligne.
    let pane = feuille.path("sheetViews/sheetView/pane").expect("volet");
    assert_eq!((pane.attr("ySplit"), pane.attr("state"), pane.attr("topLeftCell")), (Some("1"), Some("frozen"), Some("A2")));
    assert_eq!(pane.attr("xSplit"), None);
    // Une largeur par colonne, filtre automatique sur tout le tableau.
    let cols_xml: Vec<&xml::Element> = feuille.path("cols").unwrap().children_named("col").collect();
    assert_eq!(cols_xml.len(), 5);
    assert_eq!(cols_xml[4].attr("width"), Some("40"));
    assert_eq!(feuille.child("autoFilter").unwrap().attr("ref"), Some("A1:E5"));
    assert_eq!(feuille.child("dimension").unwrap().attr("ref"), Some("A1:E5"));
    assert!(feuille.child("drawing").is_none(), "pas de vignette : pas de dessin");
    assert!(c.zip().read("xl/drawings/drawing1.xml").is_none() && c.zip().read("xl/media/image1.png").is_none());
    // L'en-tête : style 1 (gras, fond gris), texte.
    let lignes = c.feuille();
    assert_eq!(lignes.len(), 5);
    let entete: Vec<(&str, &str)> = lignes[0].2.iter().map(|x| (x.1.as_str(), x.3.as_str())).collect();
    assert_eq!(entete, [("1", "N° de plan"), ("1", "Durée (images)"), ("1", "Durée (s)"), ("1", "TC entrée"), ("1", "Action")]);
    let styles = c.xml("xl/styles.xml");
    let xfs: Vec<&xml::Element> = styles.child("cellXfs").unwrap().children_named("xf").collect();
    assert_eq!(styles.child("cellXfs").unwrap().attr("count"), Some("4"));
    let police_entete: usize = xfs[1].attr("fontId").unwrap().parse().unwrap();
    assert!(styles.child("fonts").unwrap().children_named("font").nth(police_entete).unwrap().child("b").is_some(), "l'en-tête est en gras");
    assert_eq!(xfs[2].child("alignment").unwrap().attr("wrapText"), Some("1"));
    assert_eq!(xfs[3].attr("numFmtId"), Some("2"));
    // Les durées sont des nombres (pas de type « s »), le time code du texte.
    let ligne1 = &lignes[1].2;
    assert_eq!((ligne1[0].2.as_str(), ligne1[0].3.as_str()), ("s", "1"));
    assert_eq!((ligne1[1].2.as_str(), ligne1[1].3.as_str()), ("", "24"));
    assert_eq!((ligne1[2].2.as_str(), ligne1[2].3.as_str(), ligne1[2].1.as_str()), ("", "1", "3"));
    assert_eq!((ligne1[3].2.as_str(), ligne1[3].3.as_str()), ("s", "01:00:00:00"));
    // Le texte qui passe à la ligne : style 2, retour à la ligne gardé.
    assert_eq!((ligne1[4].1.as_str(), ligne1[4].3.as_str()), ("2", "Ligne 1\nLigne 2"));
    // Une cellule vide n'est pas écrite.
    assert_eq!(lignes[2].2.len(), 4);
    // Les références vont dans l'ordre : A2, B2…
    let refs: Vec<&str> = ligne1.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(refs, ["A2", "B2", "C2", "D2", "E2"]);
}

#[test]
fn xlsx_text_is_escaped_never_prefixed_and_shared() {
    let doc = film();
    let mut doc = doc;
    doc.panels[0].action = "=SOMME(A1) <b>\"gras\"</b> & 'ça' ]]>".into();
    doc.panels[1].action = "ctrl\u{1}\u{8} char\u{ffff}es".into();
    doc.panels[2].action = "=SOMME(A1) <b>\"gras\"</b> & 'ça' ]]>".into();
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Action], false);
    let (c, _) = classeur(&f, &[]);
    let lignes = c.feuille();
    assert_eq!(lignes[1].2[0].3, "=SOMME(A1) <b>\"gras\"</b> & 'ça' ]]>", "pas d'apostrophe dans le classeur : une chaîne n'est jamais une formule");
    assert_eq!(lignes[2].2[0].3, "ctrl char\u{0}es".replace('\u{0}', ""), "les caractères interdits en XML sont retirés");
    // Deux fois le même texte : une seule chaîne partagée.
    let sst = c.xml("xl/sharedStrings.xml");
    let n: usize = sst.attr("count").unwrap().parse().unwrap();
    let uniques: usize = sst.attr("uniqueCount").unwrap().parse().unwrap();
    assert_eq!(c.chaines().len(), uniques);
    assert!(uniques < n, "{uniques} chaînes pour {n} cellules");
    // Le XML brut est bien échappé (le lecteur ne s'est pas trompé en le relisant).
    let brut = String::from_utf8(c.zip().read("xl/sharedStrings.xml").unwrap()).unwrap();
    assert!(brut.contains("&lt;b&gt;") && brut.contains("&amp;") && brut.contains("&quot;gras&quot;") && !brut.contains("<b>"));
}

#[test]
fn a_text_longer_than_a_cell_can_hold_is_cut_in_the_workbook_and_the_fiche_says_so() {
    let mut doc = film();
    doc.panels[0].action = "x".repeat(ficheformats::XLSX_TEXTE_MAX + 500);
    let f = fiche_de(&doc, &[0, 1], &[Colonne::Action, Colonne::Notes], false);
    let (c, avertissements) = classeur(&f, &[]);
    let t = &c.feuille()[1].2[0].3;
    assert_eq!(t.chars().count(), ficheformats::XLSX_TEXTE_MAX);
    assert!(t.ends_with('…'));
    assert_eq!(avertissements.len(), 1);
    assert!(avertissements[0].contains("32767"), "{avertissements:?}");
    // Le CSV, lui, garde tout.
    let csv = String::from_utf8(ficheformats::csv(&f, Separateur::Virgule)[3..].to_vec()).unwrap();
    assert!(csv.contains(&"x".repeat(ficheformats::XLSX_TEXTE_MAX + 500)));
}

#[test]
fn thumbnails_are_placed_in_their_cells_with_matching_rows_and_relations() {
    let doc = film();
    let cols = [Colonne::Numero, Colonne::Vignette, Colonne::Action];
    let f = fiche_de(&doc, &tous(&doc), &cols, false);
    // Quatre vignettes, dont deux identiques : trois fichiers dans l'archive, quatre dessins. Une est plus haute que large.
    let images = vec![png(320, 180, 10), png(320, 180, 20), png(180, 320, 30), png(320, 180, 10)];
    let (c, _) = classeur(&f, &images);
    let noms: Vec<String> = c.zip().names().map(str::to_string).collect();
    let medias: Vec<&String> = noms.iter().filter(|n| n.starts_with("xl/media/")).collect();
    assert_eq!(medias.len(), 3, "l'image répétée n'est écrite qu'une fois : {noms:?}");
    for m in &medias {
        let octets = c.zip().read(m).unwrap();
        assert!(octets.starts_with(&[0x89, b'P', b'N', b'G']), "{m} est un PNG");
    }
    let feuille = c.xml("xl/worksheets/sheet1.xml");
    assert_eq!(feuille.child("drawing").unwrap().attr("r:id"), Some("rId1"));
    let rels_feuille = c.xml("xl/worksheets/_rels/sheet1.xml.rels");
    assert_eq!(rels_feuille.children_named("Relationship").next().unwrap().attr("Target"), Some("../drawings/drawing1.xml"));
    let dessin = c.xml("xl/drawings/drawing1.xml");
    let ancres: Vec<&xml::Element> = dessin.children_named("xdr:oneCellAnchor").collect();
    assert_eq!(ancres.len(), 4);
    let rels_dessin = c.xml("xl/drawings/_rels/drawing1.xml.rels");
    let cible_de = |rid: &str| rels_dessin.children_named("Relationship").find(|r| r.attr("Id") == Some(rid)).and_then(|r| r.attr("Target")).map(str::to_string);
    let lignes = c.feuille();
    for (k, a) in ancres.iter().enumerate() {
        let from = a.child("xdr:from").unwrap();
        // Colonne B (1), ligne k + 1 (en comptant l'en-tête, depuis 0).
        assert_eq!(from.child("xdr:col").unwrap().text, "1");
        assert_eq!(from.child("xdr:row").unwrap().text, (k + 1).to_string());
        let ext = a.child("xdr:ext").unwrap();
        let (cx, cy): (u64, u64) = (ext.attr("cx").unwrap().parse().unwrap(), ext.attr("cy").unwrap().parse().unwrap());
        let (w, h) = (cx / 9525, cy / 9525);
        assert!(w <= 160 && h <= 90, "{w}×{h} tient dans le cadre");
        if k == 2 {
            assert_eq!((w, h), (51, 90), "les proportions d'une image haute sont gardées");
        } else {
            assert_eq!((w, h), (160, 90));
        }
        // La ligne est assez haute pour l'image.
        let ht: f64 = lignes[k + 1].1.as_deref().expect("hauteur de ligne").parse().unwrap();
        assert!(ht >= h as f64 * 0.75, "ligne de {ht} pt pour {h} px");
        let embed = a.child("xdr:pic").unwrap().child("xdr:blipFill").unwrap().child("a:blip").unwrap().attr("r:embed").unwrap().to_string();
        let cible = cible_de(&embed).unwrap_or_else(|| panic!("{embed} sans relation"));
        assert!(noms.iter().any(|n| *n == format!("xl/{}", cible.trim_start_matches("../"))), "{cible} absent de l'archive");
        // Le chemin du fichier reste dans la cellule, sous l'image.
        assert_eq!(lignes[k + 1].2[1].3, format!("vignettes/{}", f.fichiers_vignettes()[k]));
    }
    // Les identifiants de dessin sont uniques.
    let mut ids: Vec<&str> = ancres.iter().map(|a| a.child("xdr:pic").unwrap().child("xdr:nvPicPr").unwrap().child("xdr:cNvPr").unwrap().attr("id").unwrap()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 4);
    // Les deux images identiques pointent vers le même fichier.
    let t = |k: usize| {
        let embed = ancres[k].child("xdr:pic").unwrap().child("xdr:blipFill").unwrap().child("a:blip").unwrap().attr("r:embed").unwrap().to_string();
        cible_de(&embed).unwrap()
    };
    assert_eq!(t(0), t(3));
    assert_ne!(t(0), t(1));
}

#[test]
fn a_missing_or_broken_thumbnail_leaves_its_row_plain() {
    let doc = film();
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Numero, Colonne::Vignette], false);
    // Deux images seulement, la seconde n'est pas un PNG.
    let (c, _) = classeur(&f, &[png(320, 180, 1), b"pas un png".to_vec()]);
    let dessin = c.xml("xl/drawings/drawing1.xml");
    assert_eq!(dessin.children_named("xdr:oneCellAnchor").count(), 1);
    let lignes = c.feuille();
    assert!(lignes[1].1.is_some() && lignes[2].1.is_none() && lignes[4].1.is_none());
}

#[test]
fn column_letters_and_png_sizes() {
    let noms: Vec<String> = [0usize, 1, 25, 26, 27, 51, 52, 701, 702].iter().map(|&i| ficheformats::nom_de_colonne(i)).collect();
    assert_eq!(noms, ["A", "B", "Z", "AA", "AB", "AZ", "BA", "ZZ", "AAA"]);
    assert_eq!(ficheformats::taille_png(&png(33, 17, 0)), Some((33, 17)));
    assert_eq!(ficheformats::taille_png(b"GIF89a"), None);
    assert_eq!(ficheformats::taille_png(&[]), None);
    assert_eq!(ficheformats::taille_affichee(320, 180), (160, 90));
    assert_eq!(ficheformats::taille_affichee(180, 320), (51, 90));
    assert_eq!(ficheformats::taille_affichee(10, 10), (90, 90));
}

// ------------------------------------------------------------------------------------ la tâche

fn pomper(job: &mut FicheJob, doc: &Document) {
    for _ in 0..4000 {
        job.step(doc, Duration::from_millis(40));
        if !job.running() {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("l'export ne finit pas");
}

fn fichiers_de(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    fn parcours(dir: &Path, base: &Path, v: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                parcours(&p, base, v);
            } else {
                v.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    parcours(dir, dir, &mut v);
    v.sort();
    v
}

#[test]
fn the_job_writes_the_thumbnails_then_the_workbook_and_leaves_no_part_file() {
    let doc = film();
    let dir = temp("job-xlsx");
    let cols = [Colonne::Numero, Colonne::Vignette, Colonne::TcEntree];
    let f = fiche_de(&doc, &tous(&doc), &cols, false);
    let cible = dir.join("film_fiche.xlsx");
    let mut job = FicheJob::start(&doc, f, Format::Xlsx, Separateur::PointVirgule, &cible).expect("démarré");
    pomper(&mut job, &doc);
    let rapport = job.report().cloned().expect("fini").expect("réussi");
    assert_eq!((rapport.lignes, rapport.vignettes), (4, 4));
    assert_eq!(rapport.dossier_vignettes, Some(dir.join("vignettes")));
    assert!(rapport.octets > 1000);
    assert_eq!(fichiers_de(&dir), ["film_fiche.xlsx", "vignettes/SQ01_PL001.png", "vignettes/SQ01_PL002.png", "vignettes/SQ01_PL003.png", "vignettes/SQ02_PL004.png"]);
    // Les vignettes sont de vrais PNG de la largeur choisie (le cadre du film est plus petit : pas d'agrandissement).
    let png0 = std::fs::read(dir.join("vignettes/SQ01_PL001.png")).unwrap();
    let (w, h) = ficheformats::taille_png(&png0).expect("PNG");
    assert_eq!((w, h), (64, 36));
    // Le classeur contient ces mêmes images, et se relit.
    let octets = std::fs::read(&cible).unwrap();
    let c = Classeur { zip_octets: octets };
    assert!(c.zip().names().any(|n| n == "xl/drawings/drawing1.xml"));
    assert_eq!(c.zip().names().filter(|n| n.starts_with("xl/media/")).count() > 0, true);
    assert_eq!(c.feuille().len(), 5);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_big_frame_gives_a_thumbnail_of_the_chosen_width() {
    let mut doc = Document::with_frame(1280, 720, 24);
    doc.lock_numbering();
    let dir = temp("job-grand");
    let f = fiche_de(&doc, &[0], &[Colonne::Numero, Colonne::Vignette], false);
    let cible = dir.join("g.csv");
    let mut job = FicheJob::start(&doc, f, Format::Csv, Separateur::Virgule, &cible).expect("démarré");
    pomper(&mut job, &doc);
    job.report().cloned().unwrap().expect("réussi");
    let (w, h) = ficheformats::taille_png(&std::fs::read(dir.join("vignettes/SQ01_PL001.png")).unwrap()).unwrap();
    assert_eq!((w, h), (fiche::VIGNETTE_LARGEUR, 180));
    let csv = std::fs::read_to_string(&cible).unwrap();
    assert!(csv.contains("1,vignettes/SQ01_PL001.png"), "{csv}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn without_a_thumbnail_column_no_folder_and_no_image_are_made() {
    let doc = film();
    let dir = temp("job-csv");
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Numero, Colonne::DureeImages], false);
    let cible = dir.join("film.csv");
    let mut job = FicheJob::start(&doc, f, Format::Csv, Separateur::PointVirgule, &cible).expect("démarré");
    assert!(!job.rendering() || job.running());
    pomper(&mut job, &doc);
    let rapport = job.report().cloned().unwrap().expect("réussi");
    assert_eq!((rapport.vignettes, rapport.dossier_vignettes), (0, None));
    assert_eq!(fichiers_de(&dir), ["film.csv"]);
    let csv = std::fs::read(&cible).unwrap();
    assert_eq!(&csv[..3], &[0xEF, 0xBB, 0xBF]);
    assert!(String::from_utf8_lossy(&csv).contains("1;24\r\n2;12\r\n3;36\r\n4;24\r\n"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelling_leaves_nothing_and_keeps_the_previous_file() {
    // Huit plans en 1920 × 1080 : un pas d'une milliseconde ne les dessine jamais tous.
    let mut doc = Document::with_frame(1920, 1080, 24);
    for k in 1..8 {
        doc.add_scene_after(k - 1);
    }
    doc.lock_numbering();
    let dir = temp("job-annule");
    let cible = dir.join("film_fiche.xlsx");
    std::fs::write(&cible, b"ancienne fiche").unwrap();
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Numero, Colonne::Vignette], false);
    let mut job = FicheJob::start(&doc, f, Format::Xlsx, Separateur::PointVirgule, &cible).expect("démarré");
    job.step(&doc, Duration::from_millis(1));
    job.cancel();
    assert_eq!(job.report().cloned().unwrap().err().as_deref(), Some("Export annulé."));
    assert_eq!(std::fs::read(&cible).unwrap(), b"ancienne fiche", "la fiche d'avant est intacte");
    assert_eq!(fichiers_de(&dir), ["film_fiche.xlsx"], "ni .part, ni vignettes");
    assert!(!dir.join("vignettes").exists());
    // Annuler une deuxième fois n'est rien.
    job.cancel();
    // Fermer SHOT (laisser tomber la tâche) pendant l'export : pareil.
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Numero, Colonne::Vignette], false);
    let job2 = FicheJob::start(&doc, f, Format::Xlsx, Separateur::PointVirgule, &dir.join("autre.xlsx")).expect("démarré");
    drop(job2);
    assert_eq!(fichiers_de(&dir), ["film_fiche.xlsx"]);
    // Annuler sans fichier d'avant : rien du tout.
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Numero, Colonne::Vignette], false);
    let mut job3 = FicheJob::start(&doc, f, Format::Csv, Separateur::PointVirgule, &dir.join("neuf.csv")).expect("démarré");
    job3.step(&doc, Duration::from_millis(1));
    job3.cancel();
    assert_eq!(fichiers_de(&dir), ["film_fiche.xlsx"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_old_part_file_is_replaced_and_a_missing_folder_is_refused() {
    let doc = film();
    let dir = temp("job-part");
    let cible = dir.join("a.csv");
    std::fs::write(fichejob::part_de(&cible), b"reste d'un plantage").unwrap();
    let f = fiche_de(&doc, &[0], &[Colonne::Numero], false);
    let mut job = FicheJob::start(&doc, f, Format::Csv, Separateur::PointVirgule, &cible).expect("démarré");
    pomper(&mut job, &doc);
    job.report().cloned().unwrap().expect("réussi");
    assert_eq!(fichiers_de(&dir), ["a.csv"]);
    let f = fiche_de(&doc, &[0], &[Colonne::Numero], false);
    let e = FicheJob::start(&doc, f, Format::Csv, Separateur::PointVirgule, &dir.join("absent").join("b.csv")).err().expect("refusé");
    assert!(e.contains("n'existe pas"), "{e}");
    let f = fiche_de(&doc, &[0], &[Colonne::Numero], false);
    let e = FicheJob::start(&doc, f, Format::Csv, Separateur::PointVirgule, &dir).err().expect("un dossier");
    assert!(e.contains("est un dossier"), "{e}");
    std::fs::remove_dir_all(&dir).ok();
}

/// Tient un fichier ouvert sans partage, comme Excel : personne d'autre ne peut l'écrire ni le remplacer.
#[cfg(windows)]
fn verrouille(path: &Path) -> std::fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(path).expect("verrou")
}

#[cfg(windows)]
#[test]
fn a_file_held_open_by_excel_is_refused_up_front_without_touching_it() {
    let doc = film();
    let dir = temp("job-verrou");
    let cible = dir.join("fiche.xlsx");
    std::fs::write(&cible, b"ouvert dans Excel").unwrap();
    let _excel = verrouille(&cible);
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Numero, Colonne::Vignette], false);
    let e = FicheJob::start(&doc, f, Format::Xlsx, Separateur::PointVirgule, &cible).err().expect("refusé tout de suite");
    assert!(e.contains("ouvert dans un autre programme") && e.contains("fiche.xlsx") && e.contains("Rien n'a été modifié"), "{e}");
    assert_eq!(fichiers_de(&dir), ["fiche.xlsx"], "rien n'a été écrit à côté");
    drop(_excel);
    assert_eq!(std::fs::read(&cible).unwrap(), b"ouvert dans Excel");
    std::fs::remove_dir_all(&dir).ok();
}

#[cfg(windows)]
#[test]
fn a_file_locked_during_the_export_fails_cleanly_at_the_last_step() {
    let doc = film();
    let dir = temp("job-verrou-tard");
    let cible = dir.join("fiche.xlsx");
    std::fs::write(&cible, b"avant").unwrap();
    let f = fiche_de(&doc, &tous(&doc), &[Colonne::Numero, Colonne::Vignette], false);
    let mut job = FicheJob::start(&doc, f, Format::Xlsx, Separateur::PointVirgule, &cible).expect("démarré");
    // Excel ouvre le fichier pendant que SHOT dessine les vignettes.
    let excel = verrouille(&cible);
    pomper(&mut job, &doc);
    let e = job.report().cloned().unwrap().err().expect("échec");
    assert!(e.contains("ouvert dans un autre programme") && e.contains("fiche.xlsx"), "{e}");
    drop(excel);
    assert_eq!(std::fs::read(&cible).unwrap(), b"avant", "la fiche d'avant est intacte");
    assert_eq!(fichiers_de(&dir), ["fiche.xlsx"], "ni .part, ni vignette orpheline");
    assert!(!dir.join("vignettes").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn explained_errors_tell_a_locked_file_from_another_failure() {
    let p = Path::new("C:/x/fiche.xlsx");
    let verrou = std::io::Error::from_raw_os_error(32);
    assert!(fichejob::explique(p, &verrou).contains("tableur"));
    let acces = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    assert!(fichejob::explique(p, &acces).contains("ouvert dans un autre programme"));
    let autre = std::io::Error::from(std::io::ErrorKind::NotFound);
    assert!(fichejob::explique(p, &autre).starts_with("C:/x/fiche.xlsx : "), "{}", fichejob::explique(p, &autre));
}

#[test]
fn png_thumbnails_are_composed_on_white_and_reduced() {
    use eframe::egui::Color32;
    let moitie = Color32::from_rgba_premultiplied(100, 0, 0, 128);
    let img = vec![moitie; 8 * 4];
    let octets = fichejob::png_sur_blanc(&img, [8, 4], 4).unwrap();
    let decode = image::load_from_memory(&octets).unwrap().to_rgb8();
    assert_eq!(decode.dimensions(), (4, 2));
    // 100 + (255 - 128) = 227 en rouge, 127 ailleurs : le transparent est posé sur du blanc.
    assert_eq!(decode.get_pixel(1, 1).0, [227, 127, 127]);
    assert!(fichejob::png_sur_blanc(&img, [8, 8], 4).is_err(), "pas assez de pixels");
}

// ------------------------------------------------------------------------------------ la fenêtre

fn app_de(doc: Document, nom: &str) -> (egui::Context, App, PathBuf) {
    let dir = temp(nom);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    (ctx, app, dir)
}

fn image(ctx: &egui::Context, app: &mut App) {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0))), time: Some(0.0), ..Default::default() };
    let _ = ctx.run(input, |ctx| app.fiche_window(ctx));
}

#[test]
fn the_window_opens_from_its_action_with_the_saved_columns_in_their_order() {
    let (ctx, mut app, _dir) = app_de(film(), "fenetre");
    assert!(crate::keymap::action_info("fichier.exporter_fiche").is_some_and(|a| a.implemented));
    app.settings.fiche.colonnes = vec!["notes".into(), "numero".into(), "inconnue".into(), "vignette".into()];
    app.run_action("fichier.exporter_fiche");
    for _ in 0..3 {
        image(&ctx, &mut app);
    }
    let w = app.fiche.as_ref().expect("la fenêtre reste ouverte");
    let choisies: Vec<Colonne> = w.ordre.iter().filter(|c| c.1).map(|c| c.0).collect();
    assert_eq!(choisies, [Colonne::Notes, Colonne::Numero, Colonne::Vignette]);
    assert_eq!(w.ordre.len(), Colonne::TOUTES.len(), "les autres colonnes sont là, décochées");
    assert!(!w.rendering() && !w.busy());
    // Des réglages vides : les colonnes d'origine.
    app.fiche = None;
    app.settings.fiche.colonnes = Vec::new();
    app.run_action("fichier.exporter_fiche");
    let w = app.fiche.as_ref().unwrap();
    assert_eq!(w.ordre.iter().filter(|c| c.1).map(|c| c.0).collect::<Vec<_>>(), fiche::colonnes_par_defaut());
}

#[test]
fn the_export_hub_and_the_command_bars_offer_the_fiche() {
    assert!(crate::keymap::action_info("fichier.exporter_fiche").is_some());
    let (_ctx, mut app, _dir) = app_de(film(), "hub");
    app.run_action("fichier.exporter");
    assert!(app.export_hub);
}

#[test]
fn copy_puts_the_table_on_the_clipboard_only_on_click_and_without_thumbnails() {
    let (ctx, mut app, _dir) = app_de(film(), "copie");
    app.settings.fiche.colonnes = vec!["numero".into(), "vignette".into(), "duree_images".into(), "tc_entree".into()];
    app.run_action("fichier.exporter_fiche");
    image(&ctx, &mut app);
    // Rien n'est copié tant qu'on n'a pas cliqué.
    let sortie = ctx.output(|o| o.commands.clone());
    assert!(!sortie.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(_))), "pas de copie avant le clic");
    let mut w = app.fiche.take().unwrap();
    app.fiche_copier(&ctx, &mut w);
    let copie = ctx.output(|o| o.commands.iter().find_map(|c| if let egui::OutputCommand::CopyText(t) = c { Some(t.clone()) } else { None }));
    let copie = copie.expect("texte copié");
    let lignes = lire_csv(&copie, '\t');
    assert_eq!(lignes[0], ["N° de plan", "Durée (images)", "TC entrée"], "pas de colonne de vignettes");
    assert_eq!(lignes[1], ["1", "24", "01:00:00:00"]);
    assert_eq!(lignes.len(), 5);
    assert!(w.copie.as_deref().is_some_and(|c| c.contains("4 lignes")));
}

#[test]
fn the_scope_picks_the_whole_film_the_sequence_the_chosen_plans_or_none() {
    let (_ctx, mut app, _dir) = app_de(film(), "etendue");
    app.run_action("fichier.exporter_fiche");
    let mut w = app.fiche.take().unwrap();
    w.start_tc = "01:00:00:00".into();
    w.ordre = vec![(Colonne::Numero, true)];
    let numeros = |app: &App, w: &crate::ficheui::FicheWindow| -> Vec<String> {
        let f = app.fiche_construire(w, true).expect("fiche");
        (0..f.lignes.len()).map(|k| texte(&f, k, Colonne::Numero)).collect()
    };
    w.scope = Etendue::Film;
    assert_eq!(numeros(&app, &w), ["1", "2", "3", "4"]);
    app.cur_panel = 3;
    w.scope = Etendue::Sequence;
    assert_eq!(numeros(&app, &w), ["4"]);
    app.cur_panel = 1;
    assert_eq!(numeros(&app, &w), ["1", "2", "3"]);
    app.board_sel = vec![app.doc.panels[0].id, app.doc.panels[2].id];
    w.scope = Etendue::Choisis;
    assert_eq!(numeros(&app, &w), ["1", "3"]);
    app.board_sel.clear();
    assert!(app.fiche_construire(&w, true).is_err(), "aucun plan choisi : rien à écrire");
    w.scope = Etendue::Modifies;
    w.envoi = None;
    assert!(app.fiche_construire(&w, true).is_err(), "modifiés depuis… quel envoi ?");
    // Un time code illisible est dit.
    w.scope = Etendue::Film;
    w.start_tc = "99:99".into();
    assert!(app.fiche_construire(&w, true).unwrap_err().contains("Time code de départ illisible"));
}

#[test]
fn the_window_exports_in_the_background_and_remembers_the_choices() {
    let (ctx, mut app, dir) = app_de(film(), "export-fenetre");
    app.run_action("fichier.exporter_fiche");
    let mut w = app.fiche.take().unwrap();
    w.format = Format::Csv;
    w.sep = Separateur::Virgule;
    w.par_image = true;
    w.ordre = vec![(Colonne::Numero, true), (Colonne::Vignette, true), (Colonne::TcEntree, true)];
    w.start_tc = "10:00:00:00".into();
    let cible = dir.join("sortie").join("film.csv");
    std::fs::create_dir_all(cible.parent().unwrap()).unwrap();
    app.fiche_lancer(&mut w, &cible);
    assert!(w.job.is_some(), "{:?}", w.result);
    assert!(w.busy());
    app.fiche = Some(w);
    // Tant que les vignettes ne sont pas dessinées, le projet est verrouillé (comme pour les autres exports).
    assert!(app.fiche.as_ref().is_some_and(|w| w.rendering()) && app.modal_open(), "les images vont se calculer : le projet ne bouge plus");
    // La fenêtre fait avancer l'export image après image.
    let mut fini = false;
    for _ in 0..3000 {
        image(&ctx, &mut app);
        if app.fiche.as_ref().is_some_and(|w| !w.busy()) {
            fini = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(fini, "l'export ne finit pas");
    assert!(!app.modal_open(), "le projet est libre à la fin");
    let w = app.fiche.as_ref().unwrap();
    let rapport = w.result.clone().expect("fini").expect("réussi");
    assert_eq!(rapport.lignes, 4);
    assert!(app.status.contains("Fiche technique écrite"), "{}", app.status);
    let csv = std::fs::read_to_string(&cible).unwrap();
    assert!(csv.contains("1,vignettes/SQ01_PL001.png,10:00:00:00"), "{csv}");
    assert!(cible.parent().unwrap().join("vignettes").join("SQ02_PL004.png").is_file());
    // Les choix sont gardés pour la prochaine fois.
    let r = &app.settings.fiche;
    assert_eq!((r.format, r.separateur, r.par_image, r.start_tc.as_str()), (Format::Csv, Separateur::Virgule, true, "10:00:00:00"));
    assert_eq!(r.colonnes, ["numero", "vignette", "tc_entree"]);
    assert_eq!(r.dossier.as_deref(), cible.parent());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_fiche_being_written_holds_the_tab_only_while_the_project_is_copied_and_cancelling_leaves_nothing() {
    let (ctx, mut app, dir) = app_de(film(), "onglet");
    app.run_action("fichier.exporter_fiche");
    let mut w = app.fiche.take().unwrap();
    w.ordre = vec![(Colonne::Numero, true), (Colonne::Vignette, true)];
    let cible = dir.join("film.xlsx");
    app.fiche_lancer(&mut w, &cible);
    app.fiche = Some(w);
    // Le projet n'est retenu que le temps de la copie (les premières images d'écran).
    assert_eq!(app.onglet_occupe(), Some("la copie du projet pour la fiche technique"));
    // On annule : plus rien n'occupe, rien n'est écrit.
    app.fiche.as_mut().unwrap().job.as_mut().unwrap().cancel();
    image(&ctx, &mut app);
    assert_eq!(app.onglet_occupe(), None);
    assert!(!cible.exists());
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ échantillons à ouvrir à la main

/// Écrit quatre fiches d'essai (XLSX avec vignettes, une ligne par plan ou par image, CSV à point-virgule et à
/// virgule) dans le dossier `PELURE_FICHE_ECHANTILLONS`, pour les ouvrir dans Excel, LibreOffice et Google Sheets
/// (liste de contrôle de `docs/INTERFACE.md`, partie 26) :
/// `cargo test --release --offline --target-dir target\msvc echantillons -- --ignored`.
#[test]
#[ignore]
fn echantillons_pour_ouvrir_dans_un_tableur() {
    let Some(dir) = std::env::var_os("PELURE_FICHE_ECHANTILLONS").map(PathBuf::from) else {
        eprintln!("PELURE_FICHE_ECHANTILLONS n'est pas défini : rien écrit");
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut doc = film();
    doc.panels[0].action = "Le train entre en gare.\nLes freins crissent.".into();
    doc.panels[0].dialogue = "— Quai deux, mesdames et messieurs !".into();
    doc.panels[1].action = "=1+1 (ne doit pas se calculer)".into();
    doc.panels[1].notes = "- à refaire, « guillemets », \"doubles\"; point-virgule, virgule & <chevrons>".into();
    let scene = doc.panels[2].scene;
    doc.scene_mut(scene).unwrap().name = "Arrivée".into();
    let a = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, a), CameraKey::new(23, Camera { pan: 15.0, zoom: 1.4, ..a })];
    doc.panels[2].camera = vec![CameraKey::new(0, a), CameraKey::new(35, Camera { x: a.x + 20.0, ..a })];
    doc.toggle_mark(2, 12).expect("marque");
    doc.panels[2].marks[0].action = "Gros plan sur la porte.".into();
    doc.nouvelle_version(1, 1_760_000_000).expect("V2");
    doc.panels[3].color = Some(9);
    let id = doc.set_transition(1, Kind::Dissolve).expect("fondu");
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().frames = 8;
    let voix = doc.add_sound("voix.wav".into(), "wav".into(), Arc::new(Vec::new()), Arc::new(Pcm { data: vec![100; 48_000 * 2 * 4], peaks: Vec::new() }));
    doc.add_audio_track();
    doc.add_clip(0, voix, 20, 0, 10).expect("voix");
    let plan = doc.panels[1].id;
    doc.comments.push(crate::commentaires::Comment { id: 1, panel: plan, frame: 3, text: "À refaire".into(), author: "Anthony".into(), color: [1, 2, 3], created: 0, done: false, note: None, annotation: None, follow: Default::default() });
    let mut etats = HashMap::new();
    etats.insert(doc.panels[1].id, Etat::Modifie);
    let toutes: Vec<Colonne> = Colonne::TOUTES.to_vec();
    for (nom, format, sep, par_image) in [("fiche.xlsx", Format::Xlsx, Separateur::PointVirgule, false), ("fiche_par_image.xlsx", Format::Xlsx, Separateur::PointVirgule, true), ("fiche_pv.csv", Format::Csv, Separateur::PointVirgule, false), ("fiche_virgule.csv", Format::Csv, Separateur::Virgule, true)] {
        let f = fiche::build(&doc, &Entree { plans: &tous(&doc), colonnes: &toutes, par_image, start_tc: 86_400, etats: Some(&etats), envoi: Some("v2 au réal") }).expect("fiche");
        let mut job = FicheJob::start(&doc, f, format, sep, &dir.join(nom)).expect("démarré");
        pomper(&mut job, &doc);
        job.report().cloned().unwrap().expect("réussi");
    }
    eprintln!("échantillons écrits dans {}", dir.display());
}

// ------------------------------------------------------------------------------------ envois dans l'application

#[test]
fn the_modified_scope_and_columns_follow_the_envoi_chosen_in_the_window() {
    use crate::fiabilite_fixture::dossier;
    use crate::perf_tests::banc;
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    doc.lock_numbering();
    let travail = dossier("fiche-envois");
    let mut b = banc(doc, false);
    b.app.path = Some(travail.join("Film").join("Film.pelure"));
    b.app.modified = true;
    let ctx = b.ctx.clone();
    b.app.envois_marquer(&ctx, "v1 au réal", crate::envois::lire_date("20/10/2026").unwrap());
    let calme = |b: &mut crate::perf_tests::Banc| {
        for _ in 0..800 {
            b.image(Vec::new(), 0.25);
            if b.app.envois_ui.au_repos() && !b.app.save_en_cours() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("les envois ne se calment pas");
    };
    calme(&mut b);
    // Un trait dans le plan 2.
    let id = {
        let cel = crate::document::Cel::new(b.app.doc.width, b.app.doc.height, crate::document::LayerKind::Bitmap);
        let id = b.app.doc.add_cel(cel);
        let d = b.app.doc.cels.get_mut(&id).unwrap();
        d.raster.tile_mut(3)[7] = eframe::egui::Color32::from_rgba_premultiplied(200, 10, 10, 255);
        d.version += 1;
        id
    };
    b.app.doc.panels[1].layers[0].set(0, Some(id));
    calme(&mut b);
    assert_eq!(b.app.plans_modifies(), vec![1], "le plan 2 est modifié depuis l'envoi");
    b.app.run_action("fichier.exporter_fiche");
    let mut w = b.app.fiche.take().unwrap();
    w.ordre = vec![(Colonne::Numero, true), (Colonne::Modifie, true), (Colonne::Envoi, true)];
    assert_eq!(w.envoi, Some(1), "le dernier envoi est proposé");
    w.scope = Etendue::Modifies;
    let f = b.app.fiche_construire(&w, true).expect("fiche");
    assert_eq!(f.lignes.len(), 1);
    assert_eq!((texte(&f, 0, Colonne::Numero), texte(&f, 0, Colonne::Modifie), texte(&f, 0, Colonne::Envoi)), ("2".into(), "Modifié".into(), "v1 au réal".into()));
    w.scope = Etendue::Film;
    let f = b.app.fiche_construire(&w, true).expect("fiche");
    let modifies: Vec<String> = (0..3).map(|k| texte(&f, k, Colonne::Modifie)).collect();
    assert_eq!(modifies, ["", "Modifié", ""]);
}
