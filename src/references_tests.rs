//! Tests des références visuelles : fichier à côté du manifeste et sa protection, copie des images dans
//! le projet, transformations du tableau, plans liés et filtre, image d'une vidéo selon la tête de
//! lecture, couleur prise sous la souris, annulation, cache.

use std::path::{Path, PathBuf};

use eframe::egui::{self, pos2, vec2, Rect};

use crate::app::App;
use crate::document::Document;
use crate::history::{Edit, History, Nav};
use crate::project;
use crate::references::{self, Clip, Filter, Lru, Reference, View, FILE};
use crate::referencesui::{Layout, RefSource};

fn nav() -> Nav {
    Nav { panel: 0, layer: 0 }
}

fn snap(doc: &Document) -> Edit {
    Edit::Structure { structure: doc.structure(), nav: nav() }
}

/// Trois plans de 10, 20 et 30 images.
fn film() -> Document {
    let mut doc = Document::new(32, 32, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for (p, n) in doc.panels.iter_mut().zip([10, 20, 30]) {
        p.set_length(n);
    }
    doc
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-references-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Un PNG de `w`×`h` pixels, tous de la couleur `c`.
fn write_png(path: &Path, w: u32, h: u32, c: [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(w, h, image::Rgba(c)).save(path).unwrap();
}

/// Un JPEG de `w`×`h` pixels.
fn write_jpg(path: &Path, w: u32, h: u32, c: [u8; 3]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbImage::from_pixel(w, h, image::Rgb(c)).save(path).unwrap();
}

/// Une référence d'image dont le fichier `<file>` est dans `home`.
fn reference(doc: &mut Document, home: &Path, file: &str, at: egui::Pos2) -> u64 {
    let id = doc.new_reference_id();
    write_png(&home.join(file), 8, 4, [200, 100, 50, 255]);
    let mut r = Reference::image(id, "Étude", file, [8, 4], at);
    r.home = Some(home.to_path_buf());
    doc.add_reference(r);
    id
}

fn without_home(list: &[Reference]) -> Vec<Reference> {
    list.iter().cloned().map(|r| Reference { home: None, ..r }).collect()
}

// ---------------------------------------------------------------------- le fichier

#[test]
fn the_sidecar_and_the_files_round_trip_next_to_the_manifest() {
    let work = temp("aller-retour");
    let mut doc = film();
    let (p0, p2) = (doc.panels[0].id, doc.panels[2].id);
    let a = reference(&mut doc, &work.join("attente"), "900101.png", pos2(10.0, 20.0));
    let b = reference(&mut doc, &work.join("attente"), "900102.png", pos2(-300.0, 40.0));
    {
        let r = doc.reference_mut(a).unwrap();
        r.name = "Rue de nuit".into();
        r.tags = vec!["décor".into(), "lumière".into()];
        r.rotation = 33.5;
        r.scale = 0.75;
        r.flip_h = true;
        r.grey = true;
        r.plans = vec![p0, p2];
    }
    {
        let r = doc.reference_mut(b).unwrap();
        r.video = Some(Clip { in_point: 3, out_point: Some(40), looping: true, speed: 1.5, follow: true, offset: -12, ..Clip::new(60, 25.0, "jpg") });
    }
    let dir = work.join("Film");
    let manifest = dir.join("Film.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();

    assert!(dir.join(FILE).exists(), "references.json est écrit à côté du manifeste");
    assert!(dir.join("references").join("900101.png").exists(), "le fichier est copié dans references/");
    assert!(dir.join("references").join("900102.png").exists());
    let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
    assert!(text.contains("\"references\"") && text.contains("Rue de nuit") && text.contains("\"etiquettes\""), "{text}");
    assert!(text.contains("\"boucle\": true") && text.contains("\"suivre\": true"), "{text}");

    let loaded = project::load(&manifest).unwrap();
    assert!(!loaded.warnings.iter().any(|w| w.contains(FILE) || w.contains("référence")), "{:?}", loaded.warnings);
    assert_eq!(without_home(&loaded.doc.references), without_home(&doc.references), "tout est relu, dans l'ordre de la pile");
    assert!(loaded.doc.references.iter().all(|r| r.home.as_deref() == Some(dir.join("references").as_path())), "les fichiers sont relus du projet");
    // Les numéros suivants ne retombent pas sur ceux des références relues.
    let mut doc2 = loaded.doc;
    let n = doc2.new_reference_id();
    assert!(doc2.references.iter().all(|r| r.id != n));
    std::fs::remove_dir_all(&work).ok();
}

#[test]
fn without_references_there_is_no_file_and_it_goes_when_the_last_one_does() {
    let work = temp("sans");
    let dir = work.join("Film");
    let manifest = dir.join("Film.pelure");
    let mut doc = film();
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists() && !dir.join("references").exists(), "rien à écrire sans référence");
    assert!(project::load(&manifest).unwrap().doc.references.is_empty());

    let id = reference(&mut doc, &work.join("attente"), "900111.png", pos2(0.0, 0.0));
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(dir.join(FILE).exists());
    doc.remove_reference(id);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists(), "plus de référence : le fichier part");
    // Le fichier de l'image, lui, attend : annuler la suppression après avoir enregistré le retrouve.
    assert!(dir.join("references").join("900111.png").exists(), "l'image retirée pendant la séance reste (annuler la rend)");
    std::fs::remove_dir_all(&work).ok();
}

#[test]
fn an_unreadable_or_newer_file_is_never_destroyed() {
    for (name, content) in [("abime", "{ pas du json".to_string()), ("recent", r#"{"format":"shot-references","version":99,"references":[]}"#.to_string())] {
        let work = temp(name);
        let dir = work.join("Film");
        let manifest = dir.join("Film.pelure");
        let mut doc = film();
        project::save(&doc, &manifest, &mut Default::default()).unwrap();
        std::fs::write(dir.join(FILE), &content).unwrap();
        write_png(&dir.join("references").join("900121.png"), 4, 4, [1, 2, 3, 255]);

        // À l'ouverture : le projet s'ouvre sans références, avec un avertissement.
        let loaded = project::load(&manifest).unwrap();
        assert!(loaded.doc.references.is_empty());
        assert!(loaded.warnings.iter().any(|w| w.contains(FILE)), "{name} : {:?}", loaded.warnings);

        // Enregistrer sans référence : le fichier reste tel quel, et les images avec.
        let mut doc_loaded = loaded.doc;
        project::save(&doc_loaded, &manifest, &mut Default::default()).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), content, "{name} : intact");
        assert!(dir.join("references").join("900121.png").exists(), "{name} : les images ne sont pas effacées");

        // Enregistrer avec une référence : l'ancien est mis de côté sous « .illisible », le neuf s'écrit.
        reference(&mut doc_loaded, &work.join("attente"), "900122.png", pos2(0.0, 0.0));
        project::save(&doc_loaded, &manifest, &mut Default::default()).unwrap();
        let aside = dir.join(format!("{FILE}.illisible"));
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), content, "{name} : l'ancien fichier est gardé");
        assert!(references::parse(&std::fs::read_to_string(dir.join(FILE)).unwrap()).is_ok(), "{name} : le neuf se relit");
        assert!(dir.join("references").join("900121.png").exists(), "{name} : l'image d'avant reste aussi");
        let _ = &mut doc;
        std::fs::remove_dir_all(&work).ok();
    }
}

#[test]
fn files_left_by_older_sessions_are_cleaned_but_this_sessions_are_kept() {
    let work = temp("nettoyage");
    let dir = work.join("Film");
    let manifest = dir.join("Film.pelure");
    let mut doc = film();
    let keep = reference(&mut doc, &dir.join("references"), "900131.png", pos2(0.0, 0.0));
    let gone = reference(&mut doc, &dir.join("references"), "900132.png", pos2(50.0, 0.0));
    // Un reste d'une séance d'avant (ni cité, ni touché ici), un fichier qui n'est pas de SHOT, un dossier d'images.
    write_png(&dir.join("references").join("900133.png"), 2, 2, [0, 0, 0, 255]);
    std::fs::write(dir.join("references").join("notes.txt"), "à moi").unwrap();
    write_jpg(&dir.join("references").join("900134").join("000001.jpg"), 2, 2, [0, 0, 0]);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let refs = dir.join("references");
    assert!(refs.join("900131.png").exists() && refs.join("900132.png").exists());
    assert!(!refs.join("900133.png").exists(), "le reste d'une séance d'avant est effacé");
    assert!(!refs.join("900134").exists(), "le dossier d'images d'une vidéo oubliée aussi");
    assert!(refs.join("notes.txt").exists(), "ce qui n'est pas de SHOT n'est jamais touché");
    // Retirée pendant cette séance : son fichier reste, annuler et rétablir doivent le retrouver.
    doc.remove_reference(gone);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(refs.join("900132.png").exists());
    assert!(doc.reference(keep).is_some());
    std::fs::remove_dir_all(&work).ok();
}

#[test]
fn a_damaged_entry_is_made_safe_on_reading() {
    let text = r#"{"format":"shot-references","version":1,"references":[
        {"id":0,"nom":"a","fichier":"..\\..\\Windows\\x.png","taille":[0,5],"x":1e30,"y":0,"echelle":-4,"rotation":725},
        {"id":5,"nom":"b","fichier":"5.png","taille":[10,10],"x":1,"y":2,"etiquettes":["Décor","décor"," ","a,b"],"plans":[3,3,4],
         "video":{"images":0,"ips":-1,"ext":"exe","entree":9,"sortie":2,"vitesse":99,"decalage":9999999999}},
        {"id":5,"nom":"c","fichier":"C:/etc/6.png","taille":[10,10],"x":0,"y":0}
    ]}"#;
    let list = references::parse(text).unwrap();
    assert_eq!(list.len(), 3);
    let ids: std::collections::HashSet<u64> = list.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), 3, "numéros uniques : {ids:?}");
    assert!(!ids.contains(&0));
    let a = &list[0];
    assert_eq!(a.file, "x.png", "jamais un chemin");
    assert!(a.x.abs() <= 1.0e6 && a.scale == 1.0 && (a.rotation - 5.0).abs() < 1e-3 && a.size == [1, 5], "{a:?}");
    let b = &list[1];
    assert_eq!(b.tags, vec!["Décor", "a", "b"], "sans doublon ni vide");
    assert_eq!(b.plans, vec![3, 4]);
    let c = b.video.as_ref().unwrap();
    assert_eq!((c.frames, c.ext.as_str(), c.fps), (1, "jpg", 24.0));
    assert_eq!((c.in_point, c.out_point, c.speed), (0, Some(0), references::SPEED_MAX));
    assert!(c.offset.abs() <= 10_000_000);
    assert_eq!(list[2].file, "6.png");
    // Pas le bon fichier.
    assert!(references::parse(r#"{"format":"autre","version":1}"#).is_err());
    assert!(references::parse(r#"{"format":"shot-references","version":3}"#).is_err());
}

// ---------------------------------------------------------------------- l'import

#[test]
fn importing_an_image_copies_it_into_the_project_references_folder() {
    let work = temp("import");
    let src = work.join("depart").join("photo de rue.png");
    write_png(&src, 8, 4, [10, 20, 30, 255]);
    let dest = work.join("Film").join("references");
    let imp = references::import_image(&src, &dest, 42).unwrap();
    assert_eq!((imp.file.as_str(), imp.size, imp.name.as_str()), ("42.png", [8, 4], "photo de rue"));
    assert_eq!(std::fs::read(&src).unwrap(), std::fs::read(dest.join("42.png")).unwrap(), "copiée telle quelle");
    assert!(src.exists(), "l'original n'est pas touché");
    assert!(std::fs::read_dir(&dest).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")), "pas de reste à moitié écrit");

    // Un JPEG.
    let jpg = work.join("depart").join("fond.JPEG");
    write_jpg(&jpg, 6, 3, [200, 10, 10]);
    let imp = references::import_image(&jpg, &dest, 43).unwrap();
    assert_eq!((imp.file.as_str(), imp.size), ("43.jpg", [6, 3]));

    // Ce qui n'est pas une image, ou pas lisible.
    let txt = work.join("depart").join("notes.txt");
    std::fs::write(&txt, "bonjour").unwrap();
    assert!(references::import_image(&txt, &dest, 44).unwrap_err().contains("format"));
    let fake = work.join("depart").join("faux.png");
    std::fs::write(&fake, "pas une image").unwrap();
    assert!(references::import_image(&fake, &dest, 45).unwrap_err().contains("illisible"));
    assert!(!dest.join("45.png").exists(), "rien n'est copié d'un fichier illisible");

    // Des pixels collés deviennent un PNG.
    let rgba: Vec<u8> = (0..3 * 2).flat_map(|_| [255u8, 0, 0, 255]).collect();
    let imp = references::import_rgba("Image collée", [3, 2], &rgba, &dest, 46).unwrap();
    assert_eq!((imp.file.as_str(), imp.size), ("46.png", [3, 2]));
    assert_eq!(image::image_dimensions(dest.join("46.png")).unwrap(), (3, 2));
    assert!(references::import_rgba("vide", [0, 0], &[], &dest, 47).is_err());
    std::fs::remove_dir_all(&work).ok();
}

#[test]
fn a_reference_added_before_the_first_save_waits_in_a_temporary_folder_then_follows_the_save() {
    let work = temp("attente");
    let staging = references::staging_dir();
    let mut doc = film();
    let id = reference(&mut doc, &staging, "900141.png", pos2(0.0, 0.0));
    let dir = work.join("Nouveau");
    let manifest = dir.join("Nouveau.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(dir.join("references").join("900141.png").exists(), "recopiée dans le projet à l'enregistrement");
    assert!(staging.join("900141.png").exists(), "l'original temporaire n'est pas déplacé");
    // Enregistrer sous un autre nom : les fichiers viennent de leur dossier d'attente.
    let second = work.join("Copie").join("Copie.pelure");
    project::save(&doc, &second, &mut Default::default()).unwrap();
    assert!(work.join("Copie").join("references").join("900141.png").exists());
    assert!(doc.reference(id).is_some());
    std::fs::remove_dir_all(&work).ok();
    std::fs::remove_dir_all(&staging).ok();
}

// ---------------------------------------------------------------------- le tableau

fn item(w: u32, h: u32, at: egui::Pos2, scale: f32, rotation: f32) -> Reference {
    Reference { scale, rotation, ..Reference::image(1, "t", "1.png", [w, h], at) }
}

#[test]
fn a_new_reference_gets_a_comfortable_size_on_the_board() {
    let r = Reference::image(1, "a", "1.png", [1920, 1080], pos2(0.0, 0.0));
    assert!((r.half().x * 2.0 - references::BOARD_SIDE).abs() < 0.01, "le plus long côté fait 360 unités");
    let tiny = Reference::image(2, "b", "2.png", [10, 10], pos2(0.0, 0.0));
    assert!(tiny.scale > 30.0 && tiny.scale <= references::SCALE_MAX);
    // Deux ajouts de suite ne se superposent pas.
    let spots = references::free_spot(&[r.clone()], pos2(0.0, 0.0));
    assert!((spots - pos2(0.0, 0.0)).length() >= 24.0);
}

#[test]
fn board_points_go_to_the_reference_and_back_with_rotation() {
    let r = item(200, 100, pos2(10.0, 20.0), 1.0, 90.0);
    // Les coins : le haut-gauche d'une image tournée d'un quart de tour (sens horaire) passe en haut à droite.
    let c = r.corners();
    let close = |a: egui::Pos2, b: egui::Pos2| (a - b).length() < 1e-3;
    assert!(close(c[0], pos2(60.0, -80.0)), "{:?}", c[0]);
    assert!(close(c[2], pos2(-40.0, 120.0)), "{:?}", c[2]);
    let b = r.bounds();
    assert!((b.width() - 100.0).abs() < 1e-3 && (b.height() - 200.0).abs() < 1e-3, "{b:?}");
    // Dedans / dehors suivent la rotation.
    assert!(r.contains(pos2(10.0, 110.0)));
    assert!(!r.contains(pos2(100.0, 20.0)));
    let flat = item(200, 100, pos2(10.0, 20.0), 1.0, 0.0);
    assert!(flat.contains(pos2(100.0, 20.0)) && !flat.contains(pos2(10.0, 110.0)));
    // Aller-retour du repère local.
    let r = item(300, 200, pos2(-5.0, 7.0), 0.4, 37.0);
    let v = vec2(13.0, -22.0);
    let back = r.to_local(r.from_local(v));
    assert!((back - v).length() < 1e-3);
    // Un tour complet ou presque : l'angle reste lisible.
    assert!((references::normalize_degrees(725.0) - 5.0).abs() < 1e-3);
    assert!((references::normalize_degrees(-190.0) - 170.0).abs() < 1e-3);
    assert_eq!(references::normalize_degrees(180.0), 180.0);
}

#[test]
fn dragging_moves_scales_and_rotates_from_where_the_mouse_was_pressed() {
    // Déplacer : la course d'écran est divisée par le zoom.
    let start = pos2(100.0, 50.0);
    let moved = references::dragged_center(start, vec2(40.0, -20.0), 2.0);
    assert_eq!(moved, pos2(120.0, 40.0));
    // Agrandir : la distance au centre double, l'échelle aussi ; reste bornée.
    let center = pos2(0.0, 0.0);
    assert!((references::dragged_scale(0.5, center, 100.0, pos2(200.0, 0.0)) - 1.0).abs() < 1e-5);
    assert!((references::dragged_scale(0.5, center, 100.0, pos2(0.0, -50.0)) - 0.25).abs() < 1e-5);
    assert_eq!(references::dragged_scale(1.0, center, 1.0, pos2(1.0e9, 0.0)), references::SCALE_MAX);
    // Tourner : de l'angle dont la souris a tourné autour du centre.
    let grab = (pos2(100.0, 0.0) - center).angle();
    let r = references::dragged_rotation(10.0, center, grab, pos2(0.0, 100.0), false);
    assert!((r - 100.0).abs() < 1e-3, "un quart de tour horaire : {r}");
    let snapped = references::dragged_rotation(0.0, center, grab, pos2(100.0, 9.0), true);
    assert_eq!(snapped, 0.0, "Maj : par 15 degrés");
    let snapped = references::dragged_rotation(0.0, center, grab, pos2(100.0, 40.0), true);
    assert_eq!(snapped, 15.0);
}

#[test]
fn the_view_zooms_around_the_mouse_pans_and_fits() {
    let rect = Rect::from_min_size(pos2(100.0, 50.0), vec2(800.0, 600.0));
    let mut v = View::default();
    let at = pos2(700.0, 150.0);
    let before = v.to_board(rect, at);
    v.zoom_about(rect, at, 2.0);
    assert!((v.zoom - 2.0).abs() < 1e-6);
    assert!((v.to_board(rect, at) - before).length() < 1e-3, "le point sous la souris ne bouge pas");
    assert!((v.to_board(rect, v.to_screen(rect, pos2(33.0, -4.0))) - pos2(33.0, -4.0)).length() < 1e-3);
    // Les limites du zoom.
    v.zoom_about(rect, at, 1.0e9);
    assert_eq!(v.zoom, references::ZOOM_MAX);
    v.zoom_about(rect, at, 1.0e-9);
    assert_eq!(v.zoom, references::ZOOM_MIN);
    // Glisser le fond de 100 points à zoom 2 : 50 unités du tableau.
    let mut v = View { center: pos2(0.0, 0.0), zoom: 2.0 };
    v.pan(vec2(100.0, -40.0));
    assert_eq!(v.center, pos2(-50.0, 20.0));
    // Tout voir.
    let fit = View::fit(Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 200.0)), Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0)), 0.0);
    assert!((fit.zoom - 2.0).abs() < 1e-6 && fit.center == pos2(200.0, 100.0), "{fit:?}");
    // 100 % : un pixel de l'image fait un pixel de l'écran (échelle 0,5, écran à 2 pixels par point).
    let r = item(100, 100, pos2(0.0, 0.0), 0.5, 0.0);
    assert!((View::actual_zoom(&r, 2.0) - 1.0).abs() < 1e-6);
    // Des références : la plus haute est la dernière.
    let list = vec![item(100, 100, pos2(0.0, 0.0), 1.0, 0.0), Reference { id: 2, ..item(100, 100, pos2(10.0, 0.0), 1.0, 0.0) }];
    assert_eq!(references::hit_test(list.iter(), pos2(5.0, 0.0)), Some(2));
    assert_eq!(references::hit_test(list.iter(), pos2(-45.0, 0.0)), Some(1));
    assert_eq!(references::hit_test(list.iter(), pos2(500.0, 0.0)), None);
    let all = references::bounds_of(list.iter()).unwrap();
    assert_eq!((all.min, all.max), (pos2(-50.0, -50.0), pos2(60.0, 50.0)));
    assert!(references::bounds_of(std::iter::empty()).is_none());
}

#[test]
fn the_grid_places_cells_in_rows() {
    let (first, h) = references::grid_cell(0, 7, 400.0, 100.0, 10.0);
    assert_eq!((first.min, first.size()), (pos2(0.0, 0.0), vec2(100.0, 100.0)));
    // 400 de large : trois cellules par ligne (3×100 + 2×10 = 320, quatre feraient 430).
    assert_eq!(references::grid_cell(3, 7, 400.0, 100.0, 10.0).0.min, pos2(0.0, 110.0));
    assert_eq!(references::grid_cell(6, 7, 400.0, 100.0, 10.0).0.min, pos2(0.0, 220.0));
    assert_eq!(h, 3.0 * 110.0);
    // Trop étroit : une par ligne.
    assert_eq!(references::grid_cell(2, 3, 50.0, 100.0, 10.0).0.min, pos2(0.0, 220.0));
}

#[test]
fn the_pile_order_can_be_changed() {
    let mut doc = film();
    let (a, b, c) = (doc.new_reference_id(), doc.new_reference_id(), doc.new_reference_id());
    for id in [a, b, c] {
        doc.add_reference(Reference::image(id, "r", "x.png", [4, 4], pos2(0.0, 0.0)));
    }
    assert!(doc.raise_reference(a));
    assert_eq!(doc.references.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, c, a]);
    assert!(!doc.raise_reference(a), "déjà devant");
    assert!(!doc.raise_reference(999));
}

// ---------------------------------------------------------------------- plans et filtre

#[test]
fn references_can_be_linked_to_plans_and_filtered_by_the_current_plan() {
    let mut doc = film();
    let (p0, p1, p2) = (doc.panels[0].id, doc.panels[1].id, doc.panels[2].id);
    let (a, b) = (doc.new_reference_id(), doc.new_reference_id());
    doc.add_reference(Reference::image(a, "Rue", "a.png", [4, 4], pos2(0.0, 0.0)));
    doc.add_reference(Reference::image(b, "Cuisine", "b.png", [4, 4], pos2(50.0, 0.0)));
    assert!(doc.link_reference(a, p0, true));
    assert!(!doc.link_reference(a, p0, true), "déjà liée");
    assert!(doc.link_reference(a, p1, true));
    assert!(doc.link_reference(b, p1, true));
    let only_current = Filter { current_plan: true, ..Filter::default() };
    let shown = |doc: &Document, f: &Filter, plan: u64| doc.references.iter().filter(|r| f.matches(r, Some(plan))).map(|r| r.id).collect::<Vec<_>>();
    assert_eq!(shown(&doc, &only_current, p0), vec![a]);
    assert_eq!(shown(&doc, &only_current, p1), vec![a, b]);
    assert_eq!(shown(&doc, &only_current, p2), Vec::<u64>::new(), "aucune pour un plan sans lien");
    assert_eq!(shown(&doc, &Filter::default(), p2), vec![a, b], "sans le filtre : toutes");
    assert!(!only_current.matches(&doc.references[0], None), "sans plan courant, le filtre ne laisse rien");
    // Délier.
    assert!(doc.link_reference(a, p0, false));
    assert!(!doc.link_reference(a, p0, false));
    assert_eq!(shown(&doc, &only_current, p0), Vec::<u64>::new());

    // Un plan supprimé n'est plus cité ; les références restent sur le tableau.
    let before = doc.structure();
    doc.remove_panel(1).expect("le plan est retiré");
    assert!(doc.references.iter().all(|r| !r.plans.contains(&p1)), "{:?}", doc.references.iter().map(|r| &r.plans).collect::<Vec<_>>());
    assert_eq!(doc.references.len(), 2);
    // L'annulation (la structure d'avant) les relie.
    doc.replace_structure(before);
    assert!(doc.reference(a).unwrap().plans.contains(&p1));

    // Deux plans réunis : les liens du second passent au premier.
    let mut doc = film();
    let (p0, p1) = (doc.panels[0].id, doc.panels[1].id);
    let a = doc.new_reference_id();
    doc.add_reference(Reference::image(a, "Rue", "a.png", [4, 4], pos2(0.0, 0.0)));
    doc.link_reference(a, p1, true);
    // (Les plans réunis doivent être dans la même scène.)
    let s = doc.panels[0].scene;
    doc.panels[1].scene = s;
    assert!(doc.merge_with_next(0).is_some());
    assert_eq!(doc.reference(a).unwrap().plans, vec![p0], "le plan réuni garde la référence");
}

#[test]
fn the_search_ignores_accents_and_case_and_needs_every_word() {
    let mut r = Reference::image(1, "Étude de Lumière", "1.png", [4, 4], pos2(0.0, 0.0));
    r.tags = parse("Décor, Nuit");
    let f = |text: &str, tag: Option<&str>| Filter { text: text.into(), tag: tag.map(str::to_string), current_plan: false }.matches(&r, None);
    assert!(f("", None) && f("   ", None));
    assert!(f("etude", None) && f("LUMIERE", None) && f("lumière étude", None));
    assert!(f("decor", None), "dans les étiquettes");
    assert!(!f("etude cuisine", None), "tous les mots");
    assert!(f("", Some("decor")) && f("nuit", Some("Décor")));
    assert!(!f("", Some("jour")));
    assert_eq!(references::fold("Ça Été À l'Œil"), "ca ete a l'œil");
    // Les étiquettes du tableau, sans doublon.
    let mut s = Reference::image(2, "s", "2.png", [4, 4], pos2(0.0, 0.0));
    s.tags = parse("décor, Rue");
    assert_eq!(references::all_tags(&[r, s]), vec!["Décor", "Nuit", "Rue"]);
}

fn parse(text: &str) -> Vec<String> {
    references::parse_tags(text)
}

#[test]
fn tags_are_split_on_commas_and_trimmed() {
    assert_eq!(parse(" a ,b;c,, A , "), vec!["a", "b", "c"]);
    assert!(parse("").is_empty());
    assert_eq!(parse(&"x".repeat(100))[0].chars().count(), 40);
    assert_eq!(parse(&(0..100).map(|i| format!("t{i}")).collect::<Vec<_>>().join(",")).len(), 32);
}

// ---------------------------------------------------------------------- la vidéo

fn clip(frames: usize, fps: f64) -> Clip {
    Clip::new(frames, fps, "jpg")
}

#[test]
fn the_video_frame_follows_the_playhead_with_an_offset() {
    let c = clip(100, 25.0);
    // Même cadence, sans décalage : l'image de la vidéo est celle du projet.
    for f in [0, 1, 10, 99] {
        assert_eq!(c.follow_index(f, 25), f as usize);
    }
    // Plus loin que la vidéo : sa dernière image reste.
    assert_eq!(c.follow_index(500, 25), 99);
    // Avant son début : la première.
    assert_eq!(c.follow_index(-8, 25), 0);
    // Un décalage de 5 images : la vidéo a de l'avance.
    let shifted = Clip { offset: 5, ..c.clone() };
    assert_eq!(shifted.follow_index(0, 25), 5);
    assert_eq!(shifted.follow_index(10, 25), 15);
    let late = Clip { offset: -10, ..c.clone() };
    assert_eq!(late.follow_index(4, 25), 0, "la vidéo n'a pas encore commencé");
    assert_eq!(late.follow_index(10, 25), 0);
    assert_eq!(late.follow_index(12, 25), 2);
    // Cadences différentes : on passe par le temps (25 images vidéo par seconde, projet à 24).
    assert_eq!(c.follow_index(24, 24), 25);
    assert_eq!(c.follow_index(23, 24), 23, "23/24 s = 23,95 images : la 23e");
    assert_eq!(c.follow_index(1, 24), 1);
    // La vitesse.
    let fast = Clip { speed: 2.0, ..c.clone() };
    assert_eq!(fast.follow_index(10, 25), 20);
    let slow = Clip { speed: 0.5, ..c.clone() };
    assert_eq!(slow.follow_index(10, 25), 5);
}

#[test]
fn the_video_loops_between_its_in_and_out_points() {
    let looping = Clip { in_point: 10, out_point: Some(19), looping: true, ..clip(100, 25.0) };
    assert_eq!(looping.range(), (10, 19));
    assert_eq!(looping.span(), 10);
    // La vidéo commence à son entrée, puis tourne.
    assert_eq!(looping.follow_index(0, 25), 10);
    assert_eq!(looping.follow_index(5, 25), 15);
    assert_eq!(looping.follow_index(9, 25), 19);
    assert_eq!(looping.follow_index(10, 25), 10, "retour à l'entrée");
    assert_eq!(looping.follow_index(25, 25), 15);
    assert_eq!(looping.follow_index(10_000, 25), 10);
    // Avec un décalage et une vitesse.
    let both = Clip { offset: 3, speed: 2.0, ..looping.clone() };
    assert_eq!(both.follow_index(0, 25), 16, "3 images d'avance à vitesse double : 6 images après l'entrée");
    assert_eq!(both.follow_index(2, 25), 10, "5×2 = 10 images : on a fait le tour");
    // Sans la boucle : elle s'arrête sur la sortie.
    let once = Clip { looping: false, ..looping.clone() };
    assert_eq!(once.follow_index(9, 25), 19);
    assert_eq!(once.follow_index(10, 25), 19);
    assert_eq!(once.follow_index(10_000, 25), 19);
    // Des bornes absurdes sont ramenées dans la vidéo.
    let odd = Clip { in_point: 500, out_point: Some(3), ..clip(100, 25.0) };
    assert_eq!(odd.range(), (99, 99));
    let inverted = Clip { in_point: 50, out_point: Some(20), ..clip(100, 25.0) };
    assert_eq!(inverted.range(), (50, 50));
}

#[test]
fn the_free_playback_advances_stops_or_loops() {
    let c = Clip { in_point: 2, out_point: Some(5), ..clip(10, 10.0) };
    let (pos, ended) = c.advance(2.0, 0.25);
    assert!((pos - 4.5).abs() < 1e-9 && !ended);
    assert_eq!(c.index_of(pos), 4);
    // Sans boucle : arrivée à la fin, sur la dernière image jouée.
    let (pos, ended) = c.advance(5.5, 0.2);
    assert!(ended);
    assert_eq!(c.index_of(pos), 5);
    // Avec la boucle : retour à l'entrée.
    let l = Clip { looping: true, ..c.clone() };
    let (pos, ended) = l.advance(5.5, 0.2);
    assert!(!ended);
    assert_eq!(l.index_of(pos), 3, "7,5 − 2 = 5,5 ; la boucle fait 4 images : 1,5 après l'entrée");
    // Vitesse.
    let (pos, ended) = Clip { speed: 2.0, ..clip(100, 10.0) }.advance(2.0, 0.25);
    assert!((pos - 7.0).abs() < 1e-9 && !ended, "{pos}");
}

// ---------------------------------------------------------------------- la couleur prise

/// Une image de 2×2 pixels : rouge, vert / bleu, jaune.
fn quad() -> (Vec<u8>, [usize; 2]) {
    (vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255], [2, 2])
}

#[test]
fn the_colour_under_the_mouse_follows_position_flips_and_rotation() {
    let (buf, size) = quad();
    let r = item(2, 2, pos2(100.0, 100.0), 10.0, 0.0);
    assert_eq!(r.sample(&buf, size, pos2(95.0, 95.0)), Some([255, 0, 0]), "haut-gauche");
    assert_eq!(r.sample(&buf, size, pos2(105.0, 95.0)), Some([0, 255, 0]));
    assert_eq!(r.sample(&buf, size, pos2(95.0, 105.0)), Some([0, 0, 255]));
    assert_eq!(r.sample(&buf, size, pos2(105.0, 105.0)), Some([255, 255, 0]));
    assert_eq!(r.sample(&buf, size, pos2(111.0, 100.0)), None, "hors de l'image");
    // Miroirs.
    let mh = Reference { flip_h: true, ..r.clone() };
    assert_eq!(mh.sample(&buf, size, pos2(95.0, 95.0)), Some([0, 255, 0]));
    let mv = Reference { flip_v: true, ..r.clone() };
    assert_eq!(mv.sample(&buf, size, pos2(95.0, 95.0)), Some([0, 0, 255]));
    // Un quart de tour horaire : le haut-gauche de l'image est en haut à droite.
    let turned = Reference { rotation: 90.0, ..r.clone() };
    assert_eq!(turned.sample(&buf, size, pos2(105.0, 95.0)), Some([255, 0, 0]));
    assert_eq!(turned.sample(&buf, size, pos2(105.0, 105.0)), Some([0, 255, 0]));
    // L'image décodée plus petite que l'originale : la position se ramène à sa taille.
    let big = item(1000, 1000, pos2(0.0, 0.0), 0.01, 0.0);
    assert_eq!(big.sample(&buf, size, pos2(-2.0, 3.0)), Some([0, 0, 255]));
    assert_eq!(big.pixel_at(pos2(-2.0, 3.0), [1000, 1000]), Some([300, 800]));
    // Un pixel transparent ne donne rien.
    let mut see_through = buf.clone();
    see_through[3] = 0;
    assert_eq!(r.sample(&see_through, size, pos2(95.0, 95.0)), None);
}

#[test]
fn greyscale_gives_the_value_of_each_colour() {
    assert_eq!(references::luma([0, 0, 0]), 0);
    assert_eq!(references::luma([255, 255, 255]), 255);
    for v in [1u8, 77, 128, 200, 254] {
        assert_eq!(references::luma([v, v, v]), v, "un gris reste lui-même");
    }
    assert_eq!(references::luma([255, 0, 0]), 54);
    assert_eq!(references::luma([0, 255, 0]), 182);
    assert_eq!(references::luma([0, 0, 255]), 18);
    assert_eq!(references::luma([255, 255, 0]), 237);
    // Une référence en gris donne la valeur, pas la couleur.
    let (buf, size) = quad();
    let r = Reference { grey: true, ..item(2, 2, pos2(100.0, 100.0), 10.0, 0.0) };
    assert_eq!(r.sample(&buf, size, pos2(95.0, 95.0)), Some([54, 54, 54]));
    assert_eq!(r.sample(&buf, size, pos2(105.0, 105.0)), Some([237, 237, 237]));
}

// ---------------------------------------------------------------------- le cache

#[test]
fn the_cache_drops_the_least_recently_used_and_never_what_this_frame_uses() {
    let mut c: Lru<&str, u8> = Lru::default();
    for (k, v) in [("a", 1), ("b", 2), ("c", 3)] {
        c.tick();
        c.insert(k, v, 40);
    }
    c.tick();
    assert!(c.get(&"a").is_some(), "a est revue dans l'image en cours");
    // 120 octets pour un budget de 100 : b, la moins récemment vue, part ; c et a restent.
    assert_eq!(c.evict(100), 1);
    assert!(c.contains(&"a") && !c.contains(&"b") && c.contains(&"c"));
    assert_eq!(c.total(), 80);
    // Le budget baisse encore : c part, a (utilisée dans l'image en cours) reste même hors budget.
    assert_eq!(c.evict(10), 1);
    assert!(c.contains(&"a") && c.len() == 1);
    assert_eq!(c.total(), 40);
    // Remplacer une entrée ne double pas son poids.
    c.insert("a", 9, 10);
    assert_eq!(c.total(), 10);
}

#[test]
fn the_definition_matches_what_the_screen_needs() {
    // Un petit affichage d'une grande image : une petite définition.
    assert_eq!(references::level_for(90.0, 4000, 100), 128);
    assert_eq!(references::level_for(200.0, 4000, 100), 256);
    assert_eq!(references::level_for(1500.0, 6000, 100), 2048);
    assert_eq!(references::level_for(3500.0, 6000, 100), 4096, "100 % d'une grande photo");
    assert_eq!(references::level_for(9000.0, 6000, 100), 4096, "jamais au-delà du plus grand niveau");
    assert_eq!(references::level_for(9000.0, 4000, 100), 0, "ni au-delà de l'image");
    // Une image plus petite que le niveau : telle quelle.
    assert_eq!(references::level_for(400.0, 300, 100), 0);
    // Le réglage « vignettes » du diagnostic des performances baisse la définition.
    assert_eq!(references::level_for(500.0, 4000, 50), 256);
    assert_eq!(references::level_for(500.0, 4000, 25), 128);
    // Le budget des textures : un huitième du cache de la carte graphique.
    assert_eq!(crate::referencesui::budget_bytes(1024), 128 << 20);
    assert_eq!(crate::referencesui::budget_bytes(128), 32 << 20);
    assert_eq!(crate::referencesui::budget_bytes(32768), 512 << 20);
}

#[test]
fn decoding_reduces_and_greys_off_the_main_thread() {
    let work = temp("decoder");
    let png = work.join("grande.png");
    write_png(&png, 400, 200, [200, 100, 50, 255]);
    let (w, h, rgba) = references::decode(&png, 128, false).unwrap();
    assert_eq!((w, h), (128, 64));
    assert_eq!(rgba.len(), w * h * 4);
    assert_eq!(&rgba[..4], &[200, 100, 50, 255]);
    let (w, h, _) = references::decode(&png, 0, false).unwrap();
    assert_eq!((w, h), (400, 200), "0 : tel quel");
    let (w, _, _) = references::decode(&png, 1024, false).unwrap();
    assert_eq!(w, 400, "jamais agrandie");
    let (_, _, grey) = references::decode(&png, 128, true).unwrap();
    let g = references::luma([200, 100, 50]);
    assert_eq!(&grey[..4], &[g, g, g, 255]);
    assert!(references::decode(&work.join("absente.png"), 0, false).is_none());
    std::fs::remove_dir_all(&work).ok();
}

// ---------------------------------------------------------------------- annulation

#[test]
fn adding_removing_and_moving_references_undo_with_french_step_names() {
    let mut doc = film();
    let mut hist = History::default();
    let mut nav = nav();
    let id = doc.new_reference_id();

    hist.context = Some("Ajouter une référence".into());
    hist.push(snap(&doc));
    doc.add_reference(Reference::image(id, "Rue", "900151.png", [10, 10], pos2(0.0, 0.0)));

    hist.context = Some("Déplacer la référence".into());
    hist.push(snap(&doc));
    doc.reference_mut(id).unwrap().x = 80.0;

    hist.context = Some("Supprimer la référence".into());
    hist.push(snap(&doc));
    assert!(doc.remove_reference(id));
    assert!(doc.references.is_empty());
    assert_eq!(hist.names().0, vec!["Ajouter une référence", "Déplacer la référence", "Supprimer la référence"]);

    // Annuler : la suppression, puis le déplacement, puis l'ajout.
    assert!(hist.undo(&mut doc, &mut nav));
    assert_eq!(doc.reference(id).map(|r| r.x), Some(80.0));
    assert!(hist.undo(&mut doc, &mut nav));
    assert_eq!(doc.reference(id).map(|r| r.x), Some(0.0));
    assert!(hist.undo(&mut doc, &mut nav));
    assert!(doc.references.is_empty());
    assert_eq!(hist.names().1, vec!["Ajouter une référence", "Déplacer la référence", "Supprimer la référence"]);
    // Rétablir.
    assert!(hist.redo(&mut doc, &mut nav));
    assert_eq!(doc.references.len(), 1);
    assert!(hist.redo(&mut doc, &mut nav) && hist.redo(&mut doc, &mut nav));
    assert!(doc.references.is_empty());
}

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-references-app-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    (ctx, app)
}

/// Fait tourner l'application jusqu'à ce que `done` soit vrai (les imports se font sur d'autres fils).
fn wait(app: &mut App, ctx: &egui::Context, what: &str, done: impl Fn(&App) -> bool) {
    for _ in 0..600 {
        app.tick_references(ctx);
        if done(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("{what} : trop long");
}

#[test]
fn importing_removing_and_undoing_through_the_app() {
    let work = temp("appli");
    let (ctx, mut app) = headless("import");
    let manifest = work.join("Film").join("Film.pelure");
    app.path = Some(manifest.clone());
    let src = work.join("depart").join("croquis.png");
    write_png(&src, 16, 8, [10, 200, 30, 255]);
    let notes = work.join("depart").join("notes.txt");
    std::fs::write(&notes, "x").unwrap();

    app.refs_add_paths(vec![src.clone(), notes]);
    wait(&mut app, &ctx, "import", |a| !a.doc.references.is_empty());
    assert_eq!(app.doc.references.len(), 1);
    let id = app.doc.references[0].id;
    let r = app.doc.reference(id).unwrap().clone();
    assert_eq!((r.name.as_str(), r.size), ("croquis", [16, 8]));
    let copied = work.join("Film").join("references").join(&r.file);
    assert!(copied.exists(), "copiée tout de suite dans le projet déjà enregistré");
    assert_eq!(app.refs.sel, vec![id], "la nouvelle référence est choisie");
    assert_eq!(app.history.names().0.last().copied(), Some("Ajouter une référence"));
    assert!(app.modified);
    assert!(app.status.contains("ignoré"), "le fichier qui n'est pas une image est signalé : {}", app.status);

    // Annuler / rétablir par les commandes.
    app.run_action("edition.annuler");
    assert!(app.doc.references.is_empty());
    app.run_action("edition.retablir");
    assert_eq!(app.doc.references.len(), 1);

    // Supprimer la sélection : une étape nommée, qui s'annule.
    app.refs.sel = vec![id];
    app.run_action("references.supprimer");
    assert!(app.doc.references.is_empty() && app.refs.sel.is_empty());
    assert_eq!(app.history.names().0.last().copied(), Some("Supprimer la référence"));
    app.run_action("edition.annuler");
    assert_eq!(app.doc.references.len(), 1);
    assert!(copied.exists(), "le fichier attend : annuler le retrouve");

    // Lier au plan courant, et filtrer.
    app.refs.sel = vec![id];
    app.run_action("references.lier_plan");
    let plan = app.panel().id;
    assert!(app.doc.reference(id).unwrap().plans.contains(&plan));
    assert_eq!(app.history.names().0.last().copied(), Some("Lier la référence au plan"));
    app.run_action("references.filtre_plan");
    assert!(app.refs.filter.current_plan);
    app.run_action("references.lier_plan");
    assert!(app.doc.reference(id).unwrap().plans.is_empty(), "un second appel délie");
    assert_eq!(app.history.names().0.last().copied(), Some("Délier la référence du plan"));

    // Un autre projet : le tableau repart de zéro.
    app.refs.sel = vec![id];
    app.doc_epoch += 1;
    app.tick_references(&ctx);
    assert!(app.refs.sel.is_empty());
    std::fs::remove_dir_all(&work).ok();
}

#[test]
fn pasted_pixels_become_a_reference_in_the_staging_folder_until_the_first_save() {
    let work = temp("collage");
    let (ctx, mut app) = headless("collage");
    assert!(app.path.is_none());
    let rgba: Vec<u8> = (0..4 * 4).flat_map(|_| [30u8, 60, 90, 255]).collect();
    app.refs_add_sources(vec![RefSource::Rgba { name: "Image collée".into(), size: [4, 4], rgba }]);
    wait(&mut app, &ctx, "collage", |a| !a.doc.references.is_empty());
    let r = app.doc.references[0].clone();
    let home = r.home.clone().expect("dossier d'attente");
    assert!(home.join(&r.file).exists());
    assert!(home.starts_with(std::env::temp_dir()));
    // Enregistrer : la référence suit dans le dossier du projet.
    let manifest = work.join("Collage").join("Collage.pelure");
    project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    assert!(work.join("Collage").join("references").join(&r.file).exists());
    std::fs::remove_dir_all(&work).ok();
    std::fs::remove_dir_all(&home).ok();
}

// ---------------------------------------------------------------------- le panneau

fn run_frame(ctx: &egui::Context, app: &mut App, size: egui::Vec2) {
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)), time: Some(0.0), ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.references_panel(ui));
    });
}

#[test]
fn the_panel_draws_every_layout_decodes_in_the_background_and_picks_a_colour() {
    let work = temp("panneau");
    let (ctx, mut app) = headless("panneau");
    // Vide d'abord : ne plante pas.
    for layout in [Layout::Board, Layout::Grid, Layout::List] {
        app.refs.layout = layout;
        run_frame(&ctx, &mut app, vec2(500.0, 400.0));
    }
    let home = work.join("fichiers");
    write_png(&home.join("1.png"), 64, 32, [250, 10, 20, 255]);
    let mut doc_refs = Vec::new();
    for (i, name) in ["Rue", "Cuisine", "Escalier"].iter().enumerate() {
        let id = app.doc.new_reference_id();
        let mut r = Reference::image(id, name, "1.png", [64, 32], pos2(i as f32 * 300.0, 0.0));
        r.home = Some(home.clone());
        r.tags = parse("décor");
        doc_refs.push(r);
    }
    // Un fichier manquant : un cadre qui le dit, sans planter.
    let mut broken = Reference::image(app.doc.new_reference_id(), "Perdue", "absente.png", [10, 10], pos2(0.0, 300.0));
    broken.home = Some(home.clone());
    doc_refs.push(broken);
    app.doc.references = doc_refs;

    app.refs.layout = Layout::Board;
    // Le décodage se fait sur un autre fil : on laisse tourner les images jusqu'à l'arrivée de la texture.
    let mut cached = 0;
    for _ in 0..400 {
        run_frame(&ctx, &mut app, vec2(500.0, 400.0));
        cached = app.refs.cache_bytes();
        if cached > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(cached > 0, "une texture est en mémoire graphique");
    // Une seule définition pour trois références du même fichier.
    run_frame(&ctx, &mut app, vec2(500.0, 400.0));
    assert!(app.refs.cache_bytes() <= 64 * 32 * 8 * 2, "le même fichier à la même définition ne se garde qu'une fois ({})", app.refs.cache_bytes());
    for layout in [Layout::Grid, Layout::List, Layout::Board] {
        app.refs.layout = layout;
        for _ in 0..3 {
            run_frame(&ctx, &mut app, vec2(260.0, 300.0));
        }
    }
    // Étiquette et recherche : le panneau les suit.
    app.refs.filter.text = "cuis".into();
    app.refs.filter.tag = Some("décor".into());
    run_frame(&ctx, &mut app, vec2(500.0, 400.0));

    // La pipette : au centre de la première référence, le rouge du fichier.
    app.refs.filter = Filter::default();
    let items: Vec<Reference> = app.doc.references.clone();
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 400.0));
    let view = View { center: pos2(0.0, 0.0), zoom: 1.0 };
    let mut tries = 0;
    loop {
        app.color = [0, 0, 0];
        app.refs_pick(&ctx, &items, &view, rect, view.to_screen(rect, pos2(0.0, 0.0)));
        if app.color == [250, 10, 20] || tries > 400 {
            break;
        }
        tries += 1;
        run_frame(&ctx, &mut app, vec2(500.0, 400.0));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(app.color, [250, 10, 20], "Alt+clic prend la couleur de la référence");
    assert!(app.status.contains("#FA0A14"), "{}", app.status);
    // En niveaux de gris : la valeur, pas la couleur.
    let grey = references::luma([250, 10, 20]);
    let mut items = items;
    items[0].grey = true;
    app.refs_pick(&ctx, &items, &view, rect, view.to_screen(rect, pos2(0.0, 0.0)));
    for _ in 0..400 {
        if app.color == [grey; 3] {
            break;
        }
        run_frame(&ctx, &mut app, vec2(500.0, 400.0));
        app.refs_pick(&ctx, &items, &view, rect, view.to_screen(rect, pos2(0.0, 0.0)));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(app.color, [grey; 3]);
    std::fs::remove_dir_all(&work).ok();
}

/// Une image d'écran avec des événements de souris.
fn step(ctx: &egui::Context, app: &mut App, t: &mut f64, events: Vec<egui::Event>) {
    *t += 0.05;
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(700.0, 520.0))), time: Some(*t), events, ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.references_panel(ui));
    });
}

fn moved(p: egui::Pos2) -> egui::Event {
    egui::Event::PointerMoved(p)
}

fn button(p: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

/// Un projet avec deux références sans image (le cadre qui attend suffit aux gestes), le tableau
/// dessiné une fois.
fn app_with_two_references(name: &str) -> (egui::Context, App, f64) {
    let (ctx, mut app) = headless(name);
    for (i, at) in [pos2(0.0, 0.0), pos2(500.0, 0.0)].into_iter().enumerate() {
        let id = app.doc.new_reference_id();
        app.doc.add_reference(Reference::image(id, &format!("R{i}"), "absente.png", [200, 100], at));
    }
    let mut t = 0.0;
    step(&ctx, &mut app, &mut t, vec![]);
    step(&ctx, &mut app, &mut t, vec![]);
    (ctx, app, t)
}

#[test]
fn the_mouse_moves_scales_and_rotates_a_reference_with_one_undo_step_each() {
    let (ctx, mut app, mut t) = app_with_two_references("souris");
    let (rect, view) = app.refs.geometry().expect("le tableau est dessiné");
    let first = app.doc.references[0].clone();
    let at = view.to_screen(rect, first.center());

    // Glisser la première référence de 80 points vers la droite.
    step(&ctx, &mut app, &mut t, vec![moved(at)]);
    step(&ctx, &mut app, &mut t, vec![button(at, true)]);
    assert_eq!(app.refs.sel, vec![first.id], "l'appui la choisit");
    step(&ctx, &mut app, &mut t, vec![moved(at + vec2(40.0, 0.0))]);
    step(&ctx, &mut app, &mut t, vec![moved(at + vec2(80.0, 0.0))]);
    step(&ctx, &mut app, &mut t, vec![button(at + vec2(80.0, 0.0), false)]);
    let (_, view) = app.refs.geometry().unwrap();
    let r = app.doc.reference(first.id).unwrap();
    assert!((r.x - (first.x + 80.0 / view.zoom)).abs() < 0.01 && r.y == first.y, "{} -> {} (zoom {})", first.x, r.x, view.zoom);
    assert_eq!(app.history.names().0.last().copied(), Some("Déplacer la référence"));
    assert_eq!(app.doc.references.last().map(|r| r.id), Some(first.id), "elle passe devant les autres");
    let steps = app.history.names().0.len();
    assert!(app.modified);

    // Un clic sans bouger ne fait aucune étape.
    let (rect, view) = app.refs.geometry().unwrap();
    let at = view.to_screen(rect, app.doc.reference(first.id).unwrap().center());
    step(&ctx, &mut app, &mut t, vec![button(at, true)]);
    step(&ctx, &mut app, &mut t, vec![button(at, false)]);
    assert_eq!(app.history.names().0.len(), steps);

    // Tirer le coin bas-droit vers l'extérieur : l'échelle grandit.
    let r = app.doc.reference(first.id).unwrap().clone();
    let corner = view.to_screen(rect, r.corners()[2]);
    step(&ctx, &mut app, &mut t, vec![moved(corner)]);
    step(&ctx, &mut app, &mut t, vec![button(corner, true)]);
    let out = corner + (corner - view.to_screen(rect, r.center())) * 0.5;
    step(&ctx, &mut app, &mut t, vec![moved(out)]);
    step(&ctx, &mut app, &mut t, vec![button(out, false)]);
    let bigger = app.doc.reference(first.id).unwrap();
    assert!((bigger.scale / r.scale - 1.5).abs() < 0.02, "{} -> {}", r.scale, bigger.scale);
    assert_eq!(app.history.names().0.last().copied(), Some("Redimensionner la référence"));

    // La poignée ronde, au-dessus : tourner d'un quart de tour autour du centre.
    let r = bigger.clone();
    let center = view.to_screen(rect, r.center());
    let top = view.to_screen(rect, r.from_local(vec2(0.0, -r.half().y)));
    let handle = top + (top - center).normalized() * 22.0;
    step(&ctx, &mut app, &mut t, vec![moved(handle)]);
    step(&ctx, &mut app, &mut t, vec![button(handle, true)]);
    let to = center + vec2(handle.distance(center), 0.0); // à droite du centre : 90° dans le sens horaire
    step(&ctx, &mut app, &mut t, vec![moved(to)]);
    step(&ctx, &mut app, &mut t, vec![button(to, false)]);
    let turned = app.doc.reference(first.id).unwrap();
    assert!((turned.rotation - 90.0).abs() < 1.0, "{}", turned.rotation);
    assert_eq!(app.history.names().0.last().copied(), Some("Tourner la référence"));

    // Annuler trois fois : la référence est comme au départ.
    for _ in 0..3 {
        app.run_action("edition.annuler");
    }
    let back = app.doc.reference(first.id).unwrap();
    assert_eq!((back.x, back.y, back.scale, back.rotation), (first.x, first.y, first.scale, first.rotation));
}

#[test]
fn dragging_the_background_pans_the_board_without_an_undo_step_and_escape_cancels_a_move() {
    let (ctx, mut app, mut t) = app_with_two_references("fond");
    let (rect, view) = app.refs.geometry().unwrap();
    let steps = app.history.names().0.len();
    // Un point du fond : loin des deux références.
    let empty = rect.left_bottom() + vec2(30.0, -20.0);
    assert!(app.doc.references.iter().all(|r| !r.contains(view.to_board(rect, empty))));
    step(&ctx, &mut app, &mut t, vec![moved(empty)]);
    step(&ctx, &mut app, &mut t, vec![button(empty, true)]);
    step(&ctx, &mut app, &mut t, vec![moved(empty + vec2(60.0, 0.0))]);
    step(&ctx, &mut app, &mut t, vec![moved(empty + vec2(100.0, 0.0))]);
    step(&ctx, &mut app, &mut t, vec![button(empty + vec2(100.0, 0.0), false)]);
    let (_, panned) = app.refs.geometry().unwrap();
    assert!(panned.center.x < view.center.x - 1.0, "le tableau a suivi la souris : {:?} -> {:?}", view.center, panned.center);
    assert_eq!(app.history.names().0.len(), steps, "se déplacer sur le tableau n'est pas une étape");

    // Échap pendant un déplacement : la référence revient.
    let (rect, view) = app.refs.geometry().unwrap();
    let r = app.doc.references[0].clone();
    let at = view.to_screen(rect, r.center());
    step(&ctx, &mut app, &mut t, vec![moved(at)]);
    step(&ctx, &mut app, &mut t, vec![button(at, true)]);
    step(&ctx, &mut app, &mut t, vec![moved(at + vec2(70.0, 30.0))]);
    assert_ne!(app.doc.reference(r.id).unwrap().x, r.x, "elle suit la souris pendant le geste");
    let esc = egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    step(&ctx, &mut app, &mut t, vec![esc]);
    step(&ctx, &mut app, &mut t, vec![button(at + vec2(70.0, 30.0), false)]);
    assert_eq!(app.doc.reference(r.id).unwrap().x, r.x);
    assert_eq!(app.history.names().0.len(), steps);
}

#[test]
fn shift_click_adds_to_the_selection_and_the_group_moves_together() {
    let (ctx, mut app, mut t) = app_with_two_references("groupe");
    let (rect, view) = app.refs.geometry().unwrap();
    let (a, b) = (app.doc.references[0].clone(), app.doc.references[1].clone());
    let pa = view.to_screen(rect, a.center());
    let pb = view.to_screen(rect, b.center());
    step(&ctx, &mut app, &mut t, vec![moved(pa)]);
    step(&ctx, &mut app, &mut t, vec![button(pa, true)]);
    step(&ctx, &mut app, &mut t, vec![button(pa, false)]);
    assert_eq!(app.refs.sel, vec![a.id]);
    // Maj+clic sur la seconde.
    let shift = egui::Modifiers::SHIFT;
    step(&ctx, &mut app, &mut t, vec![moved(pb)]);
    let press = egui::Event::PointerButton { pos: pb, button: egui::PointerButton::Primary, pressed: true, modifiers: shift };
    let release = egui::Event::PointerButton { pos: pb, button: egui::PointerButton::Primary, pressed: false, modifiers: shift };
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(700.0, 520.0))), time: Some(t + 0.05), modifiers: shift, events: vec![press], ..Default::default() };
    t += 0.05;
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.references_panel(ui));
    });
    step(&ctx, &mut app, &mut t, vec![release]);
    assert_eq!(app.refs.sel, vec![a.id, b.id], "les deux sont choisies, la seconde en dernier");
    // Glisser l'une des deux : les deux bougent, une seule étape.
    step(&ctx, &mut app, &mut t, vec![button(pa, true)]);
    step(&ctx, &mut app, &mut t, vec![moved(pa + vec2(0.0, 30.0))]);
    step(&ctx, &mut app, &mut t, vec![moved(pa + vec2(0.0, 60.0))]);
    step(&ctx, &mut app, &mut t, vec![button(pa + vec2(0.0, 60.0), false)]);
    let dy = 60.0 / app.refs.geometry().unwrap().1.zoom;
    for r in [&a, &b] {
        let now = app.doc.reference(r.id).unwrap();
        assert!((now.y - (r.y + dy)).abs() < 0.01 && now.x == r.x, "{} : {} -> {}", r.name, r.y, now.y);
    }
    assert_eq!(app.history.names().0.last().copied(), Some("Déplacer les références"));
    app.run_action("edition.annuler");
    assert_eq!(app.doc.reference(a.id).unwrap().y, a.y);
    assert_eq!(app.doc.reference(b.id).unwrap().y, b.y);
}

/// Écrit un petit projet de démonstration pour regarder le panneau à l'écran :
/// `cargo test --release --offline --target-dir target\msvc generate_a_demo_project -- --ignored --nocapture`.
/// Il tombe dans `target\references-demo\` (rien n'est écrit ailleurs) : des images à importer
/// (`images\`) et un projet `Demo\Demo.pelure` qui a déjà cinq références (une rampe de gris pour
/// vérifier les valeurs, des bandes de couleur pour la pipette, un paysage, un grand dessin de
/// 3000 × 1800 pixels pour voir la définition monter au zoom, une vidéo de 48 images) liées à des
/// plans, avec des étiquettes.
#[test]
#[ignore = "écrit un projet de démonstration dans target\\references-demo"]
fn generate_a_demo_project_for_the_screen() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("references-demo");
    std::fs::remove_dir_all(&root).ok();
    let images = root.join("images");
    std::fs::create_dir_all(&images).unwrap();

    // Une rampe de gris de 0 à 255, avec les valeurs 0, 64, 128, 192, 255 repérées par des barres.
    let ramp = image::RgbaImage::from_fn(512, 128, |x, y| {
        let v = (x * 255 / 511) as u8;
        if y < 14 && x % 128 == 0 {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([v, v, v, 255])
        }
    });
    ramp.save(images.join("rampe de gris.png")).unwrap();
    // Des bandes de couleur et leur dégradé : une couleur à prendre à la pipette.
    let bands = [[220u8, 40, 40], [240, 170, 30], [240, 230, 60], [50, 170, 70], [40, 110, 220], [150, 60, 200]];
    let colours = image::RgbaImage::from_fn(480, 320, |x, y| {
        let c = bands[(x as usize * bands.len() / 480).min(5)];
        let k = 1.0 - y as f32 / 640.0;
        image::Rgba([(c[0] as f32 * k) as u8, (c[1] as f32 * k) as u8, (c[2] as f32 * k) as u8, 255])
    });
    colours.save(images.join("bandes de couleur.png")).unwrap();
    // Un paysage en JPEG : ciel, collines, bâtiments, soleil.
    let land = image::RgbImage::from_fn(960, 540, |x, y| {
        let (fx, fy) = (x as f32, y as f32);
        let sky = [(120.0 + fy * 0.25) as u8, (170.0 + fy * 0.15) as u8, 235];
        let hill = 380.0 + 40.0 * (fx / 90.0).sin();
        let sun = ((fx - 760.0).powi(2) + (fy - 120.0).powi(2)).sqrt() < 46.0;
        let building = (240..330).contains(&x) && fy > 250.0 || (400..470).contains(&x) && fy > 300.0;
        if sun {
            image::Rgb([255, 240, 160])
        } else if building {
            image::Rgb([90, 80, 100])
        } else if fy > hill {
            image::Rgb([60, (120.0 + (fy - hill) * 0.3) as u8, 70])
        } else {
            image::Rgb(sky)
        }
    });
    land.save(images.join("paysage.jpg")).unwrap();
    // Un grand dessin (3000 × 1800), pour voir la définition monter quand on zoome.
    let big = image::RgbaImage::from_fn(3000, 1800, |x, y| {
        let line = (x % 150 < 3) || (y % 150 < 3);
        if line { image::Rgba([30, 30, 30, 255]) } else { image::Rgba([(x * 255 / 3000) as u8, (y * 255 / 1800) as u8, 180, 255]) }
    });
    big.save(images.join("grand dessin.png")).unwrap();

    // Une vidéo de 48 images : un carré qui traverse l'écran, le numéro de l'image dans sa couleur.
    let frames = root.join("attente").join("9001");
    std::fs::create_dir_all(&frames).unwrap();
    for k in 0..48u32 {
        let img = image::RgbImage::from_fn(320, 180, |x, y| {
            let cx = 20 + k * 6;
            if x >= cx && x < cx + 40 && (70..110).contains(&y) {
                image::Rgb([255, (k * 5) as u8, 0])
            } else {
                image::Rgb([20 + (y / 3) as u8, 30, 60])
            }
        });
        let path = crate::video::frame_file(&frames, k as usize, "jpg");
        img.save(path).unwrap();
    }

    let mut doc = Document::with_frame(1280, 720, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for p in doc.panels.iter_mut() {
        p.set_length(48);
    }
    let plans: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let mut spots = [pos2(-420.0, -160.0), pos2(0.0, -170.0), pos2(420.0, -140.0), pos2(-400.0, 190.0), pos2(30.0, 200.0)].into_iter();
    let mut add = |doc: &mut Document, name: &str, file: &str, size: [u32; 2], tags: &str, plan: Option<usize>, rot: f32| -> u64 {
        let id = doc.new_reference_id();
        let mut r = Reference::image(id, name, file, size, spots.next().unwrap_or(pos2(0.0, 0.0)));
        r.home = Some(images.clone());
        r.tags = parse(tags);
        r.rotation = rot;
        r.plans = plan.map(|i| vec![plans[i]]).unwrap_or_default();
        doc.add_reference(r);
        id
    };
    add(&mut doc, "Rampe de gris", "rampe de gris.png", [512, 128], "valeurs, outil", None, 0.0);
    add(&mut doc, "Bandes de couleur", "bandes de couleur.png", [480, 320], "couleur, outil", Some(0), 0.0);
    add(&mut doc, "Paysage", "paysage.jpg", [960, 540], "décor, lumière", Some(0), -6.0);
    add(&mut doc, "Grand dessin", "grand dessin.png", [3000, 1800], "décor", Some(1), 0.0);
    let video = add(&mut doc, "Carré en mouvement (vidéo)", "9001", [320, 180], "mouvement", Some(2), 4.0);
    {
        let r = doc.reference_mut(video).unwrap();
        r.home = Some(root.join("attente"));
        r.video = Some(Clip { looping: true, ..Clip::new(48, 24.0, "jpg") });
    }
    // (Les images à importer sont aussi dans images\ ; la vidéo attend dans attente\ jusqu'à l'enregistrement.)
    // Les références d'images doivent venir de leur dossier : `home` est `images`, donc on copie à l'enregistrement.
    let manifest = root.join("Demo").join("Demo.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let relu = project::load(&manifest).unwrap();
    assert_eq!(relu.doc.references.len(), 5);
    println!("PROJET DE DÉMONSTRATION : {}", manifest.display());
    println!("IMAGES À IMPORTER : {}", images.display());
}

#[test]
fn the_panel_is_closed_by_default_opens_on_the_right_and_has_its_actions() {
    use crate::dock::{DockLayout, Group, Pane};
    use crate::workspace::Layout as WorkLayout;
    let mut l = DockLayout::from_legacy(&WorkLayout::default());
    assert!(!l.contains(Pane::References), "fermé par défaut");
    assert_eq!(Pane::References.group(), Group::Right);
    l.open(Pane::References);
    assert!(l.contains(Pane::References));
    assert!(format!("{:?}", l.principal).contains("References"));
    // Une fenêtre à part pour un deuxième écran.
    l.detach(Pane::References);
    assert!(l.detachees.iter().any(|d| d.panneau == Pane::References));
    let back = serde_json::to_string(&l).unwrap();
    assert!(back.contains("\"references\""), "{back}");
    assert_eq!(serde_json::to_string(&Pane::References).unwrap(), "\"references\"");
    assert_eq!(Pane::References.name(), "Références");
    assert_eq!(crate::barrepanneaux::pane_de_action("vue.references"), Some(Pane::References));
    for a in ["vue.references", "references.importer_image", "references.importer_video", "references.coller", "references.supprimer", "references.lier_plan", "references.filtre_plan", "references.tout_voir"] {
        assert!(crate::keymap::action_info(a).is_some_and(|i| i.implemented), "{a}");
    }
    // Les actions n'ont pas de touche par défaut et ne gênent aucune autre.
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    assert!(map.warnings.is_empty(), "{:?}", map.warnings);
    assert!(map.keys_of("vue.references").is_empty() && map.keys_of("references.importer_image").is_empty());
}
