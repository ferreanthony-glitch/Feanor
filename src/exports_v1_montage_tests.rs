//! Essais des exports V1, montage et échange : le son dans l'EDL, le FCPXML, les marqueurs, le son NTSC sans dérive, les clés de caméra.

use std::path::PathBuf;

use crate::camera::{Camera, CameraKey};
use crate::document::Document;
use crate::editorial::{self, Fcpxml, Options};
use crate::editorial_tests::film_with_sound;
use crate::export::ExportJob;
use crate::exports_v1_tests::{court, dossier_neuf};
use crate::exportcamera::{self, CameraSettings};
use crate::transitions::Kind;

fn avec_fondu(doc: &mut Document, cut: usize, frames: usize) {
    let id = doc.set_transition(cut, Kind::Dissolve).expect("fondu enchaîné");
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().frames = frames;
}

fn commentaire(doc: &Document, plan: usize, frame: usize, text: &str, author: &str, done: bool) -> crate::commentaires::Comment {
    crate::commentaires::Comment { id: 100 + frame as u64, panel: doc.panels[plan].id, frame, text: text.into(), author: author.into(), color: [255, 0, 0], created: 0, done, note: None, annotation: None, follow: Default::default() }
}

/// Un événement d'une EDL : (canal, bobine, entrée source, sortie source, entrée ligne de temps, sortie ligne de temps).
type Evenement = (String, String, i64, i64, i64, i64);

/// Les événements d'une EDL (les coupes ; un fondu a deux lignes).
fn evenements_edl(text: &str, fps: u32) -> Vec<Evenement> {
    let tc = |s: &str| editorial::parse_timecode(s, fps).unwrap_or_else(|| panic!("time code illisible : {s}"));
    text.lines()
        .filter(|l| l.len() > 3 && l[..3].chars().all(|c| c.is_ascii_digit()))
        .map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            let n = t.len();
            (t[2].to_string(), t[1].to_string(), tc(t[n - 4]), tc(t[n - 3]), tc(t[n - 2]), tc(t[n - 1]))
        })
        .collect()
}

#[test]
fn the_edl_carries_the_sound_tracks() {
    let doc = film_with_sound(24);
    let o = Options { sound: true, ..Options::default() };
    let plan = editorial::build_plan(&doc, &o, 0, "Mon film").unwrap();
    assert_eq!(plan.audio.len(), 3);
    let texte = editorial::write_edl(&plan);
    let ev = evenements_edl(&texte, 24);
    let video: Vec<_> = ev.iter().filter(|e| e.0 == "V").collect();
    let son: Vec<_> = ev.iter().filter(|e| e.0 != "V").collect();
    assert_eq!(video.len(), 4, "les quatre plans");
    assert_eq!(son.len(), 3, "les trois extraits");
    // Piste 1 (la voix) : canal A, deux extraits ; piste 2 (la musique) : canal A2.
    assert_eq!(son.iter().map(|e| e.0.as_str()).collect::<Vec<_>>(), ["A", "A", "A2"]);
    for (e, a) in son.iter().zip(&plan.audio) {
        assert_eq!(e.2, 0, "un fichier son commence à zéro");
        assert_eq!(e.3, a.len as i64);
        assert_eq!(e.4, 86_400 + a.start as i64, "place dans la ligne de temps");
        assert_eq!(e.5, 86_400 + (a.start + a.len) as i64);
    }
    let reels: Vec<&str> = ev.iter().map(|e| e.1.as_str()).collect();
    assert!(reels.iter().all(|r| r.len() <= 8), "bobines de 8 caractères au plus");
    assert!(texte.contains("* PISTE 1 : Voix e") && texte.contains("* PISTE 2 : Musique"), "{texte}");
    assert!(texte.contains("* FROM CLIP NAME: A1_Voix_e_01"), "{texte}");
    // Sans le son : l'EDL d'avant.
    let sans = editorial::build_plan(&doc, &Options { sound: false, ..Options::default() }, 0, "Mon film").unwrap();
    assert!(evenements_edl(&editorial::write_edl(&sans), 24).iter().all(|e| e.0 == "V"));
}

/// Des images, d'après un temps `N/Ds` de FCPXML et la durée d'une image (`frameDuration`).
fn temps_en_images(t: &str, frame_duration: &str) -> i64 {
    let frac = |s: &str| -> (i64, i64) {
        let s = s.trim_end_matches('s');
        match s.split_once('/') {
            Some((n, d)) => (n.parse().unwrap(), d.parse().unwrap()),
            None => (s.parse().unwrap(), 1),
        }
    };
    let ((n, d), (a, b)) = (frac(t), frac(frame_duration));
    n * b / (d * a)
}

#[test]
fn fcpxml_is_well_formed_and_agrees_with_the_edl_and_the_other_files() {
    for (ntsc, version) in [(false, Fcpxml::V1_10), (true, Fcpxml::V1_9)] {
        let mut doc = film_with_sound(24);
        avec_fondu(&mut doc, 1, 12);
        let (c1, c2) = (commentaire(&doc, 2, 7, "Raccord à vérifier", "Anne", false), commentaire(&doc, 2, 9, "Déjà réglé", "Anne", true));
        doc.comments.extend([c1, c2]);
        let o = Options { sound: true, fcpxml: true, fcpxml_version: version, ntsc, ..Options::default() };
        let plan = editorial::build_plan(&doc, &o, 0, "Mon film é & <x>").unwrap();
        let dir = PathBuf::from("C:/Exports/Mon film");
        let texte = editorial::write_fcpxml(&plan, &dir, version);
        let racine = crate::xml::parse(&texte).unwrap_or_else(|e| panic!("FCPXML mal formé : {e}\n{texte}"));
        assert_eq!(racine.name, "fcpxml");
        assert_eq!(racine.attr("version"), Some(version.label()));
        let res = racine.child("resources").expect("resources");
        let format = res.child("format").expect("format");
        let fd = format.attr("frameDuration").unwrap().to_string();
        assert_eq!(fd, if ntsc { "1001/24000s" } else { "1/24s" });
        assert_eq!((format.attr("width"), format.attr("height")), (Some("64"), Some("36")));
        // Les médias : un par plan, un par extrait son, chacun avec son adresse absolue (attribut en 1.9, élément en 1.10).
        let assets: Vec<_> = res.children_named("asset").collect();
        assert_eq!(assets.len(), plan.clips.len() + plan.audio.len());
        for a in &assets {
            let src = match version {
                Fcpxml::V1_9 => a.attr("src").expect("src").to_string(),
                Fcpxml::V1_10 => {
                    assert!(a.attr("src").is_none());
                    let m = a.child("media-rep").expect("media-rep");
                    assert_eq!(m.attr("kind"), Some("original-media"));
                    m.attr("src").unwrap().to_string()
                }
            };
            assert!(src.starts_with("file:///C:/Exports/Mon%20film/") && (src.contains("/media/") || src.contains("/audio/")), "{src}");
            assert!(src.ends_with(".mov") || src.ends_with(".wav"), "{src}");
        }
        assert!(res.child("effect").is_some_and(|e| e.attr("name") == Some("Cross Dissolve")), "l'effet du fondu enchaîné");
        // La séquence : durée, début ; ses clips sont ceux de l'EDL.
        let seq = racine.path("library/event/project/sequence").expect("sequence");
        assert_eq!(temps_en_images(seq.attr("duration").unwrap(), &fd), plan.total as i64);
        assert_eq!(temps_en_images(seq.attr("tcStart").unwrap(), &fd), plan.start_tc);
        assert_eq!(racine.path("library/event").unwrap().attr("name"), Some("Mon film é & <x>"), "les caractères spéciaux sont décodés");
        let spine = seq.child("spine").expect("spine");
        let clips: Vec<_> = spine.children_named("asset-clip").collect();
        assert_eq!(clips.len(), plan.clips.len());
        let edl = evenements_edl(&editorial::write_edl(&plan), 24);
        let edl_video: Vec<&Evenement> = edl.iter().filter(|e| e.0 == "V" && e.4 != e.5).collect();
        let ids: Vec<&str> = assets.iter().map(|a| a.attr("id").unwrap()).collect();
        for (k, (c, p)) in clips.iter().zip(&plan.clips).enumerate() {
            assert_eq!(c.attr("name"), Some(p.name.as_str()));
            assert!(ids.contains(&c.attr("ref").unwrap()), "le média existe");
            let (offset, start, duration) = (temps_en_images(c.attr("offset").unwrap(), &fd), temps_en_images(c.attr("start").unwrap(), &fd), temps_en_images(c.attr("duration").unwrap(), &fd));
            assert_eq!((offset - plan.start_tc, start, duration), (p.start as i64, p.head as i64, p.len as i64), "{}", p.name);
            // Coupes franches : mêmes entrées et sorties que l'EDL (autour du fondu, l'EDL les raccourcit de ce qu'il mange).
            if plan.dissolves.iter().all(|d| d.a != k && d.b != k) {
                let e = edl_video.iter().find(|e| e.4 == offset).unwrap_or_else(|| panic!("{} : événement de l'EDL à {offset}", p.name));
                assert_eq!(e.5, offset + duration, "{}", p.name);
            }
        }
        // Le fondu enchaîné : après le clip sortant, de la durée demandée.
        let t: Vec<_> = spine.children_named("transition").collect();
        assert_eq!(t.len(), 1);
        assert_eq!(temps_en_images(t[0].attr("duration").unwrap(), &fd), 12);
        assert_eq!(t[0].child("filter-video").unwrap().attr("ref"), res.child("effect").unwrap().attr("id"));
        let ordre: Vec<&str> = spine.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(ordre.iter().position(|n| *n == "transition"), Some(plan.dissolves[0].a + 1), "la transition suit le clip sortant : {ordre:?}");
        // Le son : accroché aux clips image, sur des voies négatives, au bon endroit de la ligne de temps.
        let mut sons = Vec::new();
        for (k, c) in clips.iter().enumerate() {
            let p = &plan.clips[k];
            for a in c.children_named("asset-clip") {
                let lane: i32 = a.attr("lane").unwrap().parse().unwrap();
                assert!(lane < 0);
                let pos = temps_en_images(a.attr("offset").unwrap(), &fd) - p.head as i64 + p.start as i64;
                sons.push((a.attr("name").unwrap().to_string(), -lane, pos, temps_en_images(a.attr("duration").unwrap(), &fd)));
            }
        }
        assert_eq!(sons.len(), plan.audio.len());
        for a in &plan.audio {
            assert!(sons.iter().any(|s| s.0 == a.name && s.1 == a.track as i32 && s.2 == a.start as i64 && s.3 == a.len as i64), "{} : {sons:?}", a.name);
        }
        // Les marqueurs : le numéro de chaque plan et le commentaire non résolu (jamais le résolu).
        let mut marqueurs = Vec::new();
        for c in &clips {
            for m in c.children_named("marker") {
                marqueurs.push(m.attr("value").unwrap().to_string());
            }
        }
        assert_eq!(marqueurs.len(), plan.clips.len() + 1, "{marqueurs:?}");
        for p in &plan.clips {
            assert!(marqueurs.contains(&p.name), "{} : {marqueurs:?}", p.name);
        }
        assert!(marqueurs.iter().any(|m| m.contains("Raccord à vérifier") && m.contains("Anne")), "{marqueurs:?}");
        assert!(!marqueurs.iter().any(|m| m.contains("Déjà réglé")));
    }
}

#[test]
fn markers_are_written_in_the_xml_and_the_otio_and_can_be_switched_off() {
    let mut doc = film_with_sound(24);
    let c = commentaire(&doc, 1, 4, "Plus lent", "", false);
    doc.comments.push(c);
    let plan = editorial::build_plan(&doc, &Options { sound: false, ..Options::default() }, 0, "Mon film").unwrap();
    assert_eq!(plan.markers.len(), 5, "quatre numéros de plan et un commentaire");
    let m = plan.markers.iter().find(|m| !m.note.is_empty()).unwrap();
    assert_eq!((m.clip, m.frame, m.note.as_str(), m.name.as_str()), (1, 4, "Plus lent", "Commentaire"));
    // XML : des « marker » dans les clips, en images du fichier (poignées comprises).
    let xml = crate::xml::parse(&editorial::write_xml(&plan, None)).unwrap();
    let mut tous = Vec::new();
    xml.find_all("marker", &mut tous);
    assert_eq!(tous.len(), 5);
    let c = tous.iter().find(|m| m.child("comment").is_some()).unwrap();
    assert_eq!(c.child("comment").unwrap().text.trim(), "Plus lent");
    assert_eq!(c.child("in").unwrap().text.trim(), (plan.clips[1].head + 4).to_string());
    // OTIO : des marqueurs dans les clips.
    let otio: serde_json::Value = serde_json::from_str(&editorial::write_otio(&plan, None)).unwrap();
    let clip1 = &otio["tracks"]["children"][0]["children"][1];
    let marqueurs = clip1["markers"].as_array().unwrap();
    assert_eq!(marqueurs.len(), 2);
    assert_eq!(marqueurs[1]["OTIO_SCHEMA"], "Marker.2");
    assert_eq!(marqueurs[1]["comment"], "Plus lent");
    assert_eq!(marqueurs[1]["marked_range"]["start_time"]["value"], (plan.clips[1].head + 4) as f64);
    // Décoché : aucun marqueur.
    let sans = editorial::build_plan(&doc, &Options { sound: false, markers: false, ..Options::default() }, 0, "Mon film").unwrap();
    assert!(sans.markers.is_empty());
    let mut aucun = Vec::new();
    let sans_xml = crate::xml::parse(&editorial::write_xml(&sans, None)).unwrap();
    sans_xml.find_all("marker", &mut aucun);
    assert!(aucun.is_empty());
}

#[test]
fn ntsc_sound_is_slowed_by_a_tenth_of_a_percent_to_stay_in_sync() {
    // Un son qui monte en rampe : le ralenti de 0,1 % le rend exactement (l'interpolation cubique est exacte sur une droite).
    let rampe: Vec<[f32; 2]> = (0..48_000).map(|i| [i as f32 / 100_000.0, -(i as f32) / 100_000.0]).collect();
    let lent = editorial::slow_for_ntsc(&rampe);
    assert_eq!(lent.len(), 48_048, "durée × 1001/1000");
    for j in [0usize, 1, 1000, 24_000, 47_000] {
        let t = j as f64 * 1000.0 / 1001.0;
        assert!((lent[j][0] as f64 - t / 100_000.0).abs() < 1e-5, "{j}");
        assert!((lent[j][1] as f64 + t / 100_000.0).abs() < 1e-5, "{j}");
    }
    // Un son constant reste constant.
    let plat = editorial::slow_for_ntsc(&vec![[0.25, -0.5]; 1000]);
    assert!(plat.iter().all(|s| (s[0] - 0.25).abs() < 1e-6 && (s[1] + 0.5).abs() < 1e-6) && plat.len() == 1001);
    // Dans l'export : le son d'un film NTSC dure exactement autant que ses images (23,976), pas autant que celles du projet (24).
    for ntsc in [false, true] {
        let doc = film_with_sound(24);
        let plan = editorial::build_plan(&doc, &Options { sound: true, ntsc, ..Options::default() }, 0, "x").unwrap();
        assert!(plan.audio_sources.iter().all(|s| s.ntsc == ntsc));
        for (a, src) in plan.audio.iter().zip(&plan.audio_sources) {
            let octets = editorial::wav_bytes(src);
            let echantillons = ((octets.len() - 44) / 4) as f64;
            let secondes = echantillons / 48_000.0;
            let attendu = a.len as f64 * if ntsc { 1001.0 / 24_000.0 } else { 1.0 / 24.0 };
            assert!((secondes - attendu).abs() < 2.0 / 48_000.0, "{} : {secondes} s au lieu de {attendu} s (ntsc {ntsc})", a.name);
        }
    }
}

// ---------------------------------------------------------------------------------------- caméra

#[test]
fn camera_keys_are_exported_as_json_and_csv() {
    let mut doc = crate::editorial_tests::film(24);
    let c = doc.default_camera();
    doc.panels[0].camera = vec![
        CameraKey::new(0, c),
        CameraKey { ease_in: 4.0, ease_out: 2.0, ..CameraKey::new(12, Camera { x: c.x + 100.0, y: c.y - 20.0, zoom: 1.5, angle: 7.5, dolly: 12.0, tilt: -3.0, pan: 4.0 }) },
        CameraKey { smooth: true, ..CameraKey::new(23, Camera { x: c.x + 50.0, zoom: 2.0, ..c }) },
    ];
    let s = CameraSettings::default();
    let e = exportcamera::build(&doc, &[0, 1], "Mon film", &s);
    let v: serde_json::Value = serde_json::from_str(&e.json).expect("JSON");
    assert_eq!((v["format"].as_str(), v["version"].as_i64(), v["cadence"].as_i64(), v["projet"].as_str()), (Some("shot-camera"), Some(1), Some(24), Some("Mon film")));
    assert_eq!(v["cadre"], serde_json::json!({ "largeur": 64, "hauteur": 36 }));
    assert!(v["unites"]["zoom"].as_str().unwrap().contains("taille de l'image finale") && v["unites"]["rotation"].as_str().unwrap().contains("horaire"));
    let plans = v["plans"].as_array().unwrap();
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0]["plan"], "SQ01_PL001");
    assert_eq!((plans[0]["image_debut"].as_i64(), plans[0]["images"].as_i64()), (Some(0), Some(24)));
    assert_eq!(plans[1]["image_debut"], 24);
    let cles = plans[0]["cles"].as_array().unwrap();
    assert_eq!(cles.len(), 3);
    assert_eq!(cles[1]["image"], 12);
    assert_eq!((cles[1]["x"].as_f64(), cles[1]["y"].as_f64()), (Some((c.x + 100.0) as f64), Some((c.y - 20.0) as f64)));
    assert_eq!(
        (cles[1]["zoom"].as_f64(), cles[1]["rotation"].as_f64(), cles[1]["avancee"].as_f64(), cles[1]["bascule"].as_f64(), cles[1]["panoramique"].as_f64()),
        (Some(1.5), Some(7.5), Some(12.0), Some(-3.0), Some(4.0))
    );
    assert_eq!((cles[0]["interpolation"].as_str(), cles[1]["interpolation"].as_str(), cles[2]["interpolation"].as_str()), (Some("lineaire"), Some("adoucie"), Some("adoucie")));
    assert_eq!((cles[1]["adouci_entree"].as_f64(), cles[1]["adouci_sortie"].as_f64(), cles[2]["adouci_ancien"].as_bool()), (Some(4.0), Some(2.0), Some(true)));
    // Un plan sans clé : aucune clé, mais la caméra de chaque image.
    assert!(plans[1]["cles"].as_array().unwrap().is_empty());
    // La caméra de chaque image est celle du film.
    let par_image = plans[0]["par_image"].as_array().unwrap();
    assert_eq!(par_image.len(), 24);
    for f in [0usize, 5, 12, 20, 23] {
        let cam = doc.panel_camera(0, f);
        assert_eq!(par_image[f]["image"], f);
        assert_eq!(par_image[f]["x"].as_f64(), Some(cam.x as f64));
        assert_eq!(par_image[f]["zoom"].as_f64(), Some(cam.zoom as f64));
        assert_eq!(par_image[f]["rotation"].as_f64(), Some(cam.angle as f64));
    }
    assert_eq!(plans[1]["par_image"].as_array().unwrap().len(), 12);
    // Les CSV : une ligne par clé, une par image.
    assert_eq!(e.keys_csv.lines().count(), 1 + 3);
    assert!(e.keys_csv.lines().next().unwrap().starts_with("plan,titre,image,x,y,zoom,rotation"));
    assert!(e.keys_csv.lines().nth(2).unwrap().contains(",12,") && e.keys_csv.contains("adoucie"));
    assert_eq!(e.frames_csv.as_ref().unwrap().lines().count(), 1 + 24 + 12);
    // Sans la caméra de chaque image.
    let court = exportcamera::build(&doc, &[0], "x", &CameraSettings { per_frame: false, ..s });
    assert!(serde_json::from_str::<serde_json::Value>(&court.json).unwrap()["plans"][0].get("par_image").is_none() && court.frames_csv.is_none());
}

#[test]
fn the_camera_export_writes_its_files_in_the_background() {
    let doc = crate::editorial_tests::film(24);
    let dir = dossier_neuf("camera");
    let out = dir.join("film.json");
    let s = CameraSettings::default();
    let target = out.clone();
    let mut job = ExportJob::task("Clés de caméra".into(), &out, "ok".into(), move |snap, _| {
        let e = exportcamera::build(snap, &[0, 1, 2, 3], "Film", &s);
        exportcamera::write(&e, &target, &s).map(|_| ())
    });
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let mut noms: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    noms.sort();
    assert_eq!(noms, ["film.json", "film_cles.csv", "film_images.csv"]);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&out).unwrap()).unwrap()["plans"].as_array().unwrap().len(), 4);
    std::fs::remove_dir_all(&dir).ok();
}
