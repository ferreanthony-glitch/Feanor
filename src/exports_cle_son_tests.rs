//! Essais de la partie 105 : les images à la clé (composées et par calque) et le son par plan. Chaque essai relit les fichiers écrits.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::document::{Document, Layer, LayerKind};
use crate::editorial_tests::film_with_sound;
use crate::export::{self, ExportJob};
use crate::exportcles;
use crate::exportsonplan;
use crate::exportstems::{AudioFormat, Container, Depth};
use crate::exportstills::{StillFormat, StillsPer, StillsSettings};
use crate::exports_v1_tests::{court, dossier_neuf};
use crate::fxcache::monde;
use crate::raster::unpremultiply;
use crate::render;

// ----------------------------------------------------------------------------------------- les clés

/// Un dessin de la feuille entière : des bandes de 2 lignes sur 4 de la couleur `c`, le reste transparent.
fn dessin(doc: &mut Document, c: Color32) -> u64 {
    dessin_phase(doc, c, 0)
}

/// Comme `dessin`, les bandes décalées de `phase` lignes (deux calques ne se couvrent pas tout à fait).
fn dessin_phase(doc: &mut Document, c: Color32, phase: usize) -> u64 {
    let id = doc.new_cel(LayerKind::Bitmap);
    let (w, h) = (doc.width, doc.height);
    let r = &mut doc.cels.get_mut(&id).unwrap().raster;
    for y in 0..h {
        if (y + 4 - phase) % 4 < 2 {
            for x in 0..w {
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % 64) * 64 + x % 64] = c;
            }
        }
    }
    id
}

fn calque(nom: &str, frames: Vec<Option<u64>>) -> Layer {
    let mut l = Layer::new(nom, LayerKind::Bitmap);
    l.frames = frames;
    l
}

/// Deux plans. Plan 1 (10 images) : « Perso » a un dessin sur les images 0-3 et un autre sur 4-9 ; « Fond » un dessin à l'image 2 seule, qui
/// revient à l'image 7 (même dessin après un vide : une nouvelle clé). Clés attendues : images 0, 2, 4, 7. Plan 2 (5 images) : un dessin tenu
/// sur les 5 images, une seule clé. Autour : un calque caché, une annotation, un calque d'effet et un calque vidéo, qui ne font aucune clé.
fn projet_cles() -> Document {
    let mut doc = Document::with_frame(64, 36, 12);
    doc.add_scene_after(0);
    let (p1, p2) = (dessin(&mut doc, Color32::from_rgb(200, 40, 40)), dessin(&mut doc, Color32::from_rgb(40, 40, 200)));
    let fond = dessin_phase(&mut doc, Color32::from_rgb(40, 160, 60), 2);
    let tenu = dessin(&mut doc, Color32::from_rgb(220, 200, 20));
    let cache = dessin(&mut doc, Color32::from_rgb(10, 10, 10));
    let mut perso = vec![Some(p1); 4];
    perso.extend(vec![Some(p2); 6]);
    let mut f = vec![None; 10];
    f[2] = Some(fond);
    f[7] = Some(fond);
    let mut cachee = calque("Caché", vec![None, Some(cache), None, None, None, None, None, None, None, None]);
    cachee.visible = false;
    let mut note = calque("Note", vec![None, None, None, Some(cache), None, None, None, None, None, None]);
    note.annotation = true;
    let mut video = calque("Rushes", vec![None, None, None, None, None, Some(cache), None, None, None, None]);
    video.video = Some(crate::video::VideoClip { id: 7, name: "v.mp4".into(), frames: 10, fps: 12.0, start: 0, ext: "jpg".into() });
    doc.panels[0].layers = vec![calque("Fond", f), calque("Perso", perso), cachee, note, video];
    doc.panels[0].length = 10;
    doc.panels[1].layers = vec![calque("Fond", vec![Some(tenu); 5])];
    doc.panels[1].length = 5;
    doc
}

fn fichiers(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn a_key_is_a_drawing_at_the_frame_where_it_starts_on_a_visible_layer() {
    let doc = projet_cles();
    let cles = exportcles::cles_du_plan(&doc, 0, false);
    assert_eq!(cles.iter().map(|c| (c.numero, c.frame, c.layers.clone())).collect::<Vec<_>>(), [(1, 0, vec![1]), (2, 2, vec![0]), (3, 4, vec![1]), (4, 7, vec![0])], "le même dessin après un vide recommence une clé ; ni caché, ni annotation, ni vidéo");
    assert_eq!(exportcles::cles_du_plan(&doc, 1, false).len(), 1, "un dessin tenu sur 5 images : une clé");
    // Avec les annotations : celle de l'image 3 compte.
    let avec = exportcles::cles_du_plan(&doc, 0, true);
    assert_eq!(avec.iter().map(|c| c.frame).collect::<Vec<_>>(), [0, 2, 3, 4, 7]);
    assert_eq!(exportcles::compter(&doc, &[0, 1], false), (5, 0));
    assert_eq!(exportcles::compter_calques(&doc, &[0, 1], false), 5);
    assert_eq!(exportcles::largeur(5), 2);
    assert_eq!(exportcles::largeur(99), 2);
    assert_eq!(exportcles::largeur(100), 3);
}

#[test]
fn key_files_are_numbered_by_plan_then_key_and_sort_well() {
    let doc = projet_cles();
    let items = exportcles::items(&doc, &[0, 1], false);
    let noms: Vec<&str> = items.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(noms, ["SQ01_PL001_K01", "SQ01_PL001_K02", "SQ01_PL001_K03", "SQ01_PL001_K04", "SQ01_PL002_K01"]);
    assert_eq!(items.iter().map(|s| s.frame).collect::<Vec<_>>(), [(0, 0), (0, 2), (0, 4), (0, 7), (1, 0)]);
    let mut tries = noms.clone();
    tries.sort_unstable();
    assert_eq!(tries, noms, "l'ordre alphabétique est plan, puis clé");
    // Par calque : le numéro est celui de la clé du plan, le nom du calque suit.
    let c = exportcles::items_calques(&doc, &[0, 1], false);
    assert_eq!(c.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["SQ01_PL001_K01_Perso", "SQ01_PL001_K02_Fond", "SQ01_PL001_K03_Perso", "SQ01_PL001_K04_Fond", "SQ01_PL002_K01_Fond"]);
    // Plus de 99 clés : trois chiffres pour tout l'export.
    let mut longue = projet_cles();
    let (a, b) = (dessin(&mut longue, Color32::RED), dessin(&mut longue, Color32::BLUE));
    longue.panels[0].layers[1].frames = (0..120).map(|k| Some(if k % 2 == 0 { a } else { b })).collect();
    longue.panels[0].length = 120;
    let noms: Vec<String> = exportcles::items(&longue, &[0, 1], false).into_iter().map(|s| s.name).collect();
    assert!(noms[0] == "SQ01_PL001_K001" && noms.contains(&"SQ01_PL002_K001".to_string()), "{noms:?}");
}

#[test]
fn the_composed_key_export_writes_exactly_the_key_frames_and_leaves_the_project_alone() {
    monde::lire();
    let doc = projet_cles();
    let avant: Vec<Vec<Layer>> = doc.panels.iter().map(|p| p.layers.clone()).collect();
    let items = exportcles::items(&doc, &[0, 1], false);
    let dir = dossier_neuf("cles-composees");
    let s = StillsSettings { per: StillsPer::Cle, transparent: true, ..StillsSettings::default() };
    let mut job = exportcles::job(&dir, &items, doc.frame, &s, false);
    court(&mut job, &doc);
    assert!(matches!(&job.outcome, Some(Ok(m)) if m.contains("5 images à la clé")), "{:?}", job.outcome);
    assert_eq!(fichiers(&dir), ["SQ01_PL001_K01.png", "SQ01_PL001_K02.png", "SQ01_PL001_K03.png", "SQ01_PL001_K04.png", "SQ01_PL002_K01.png"], "ni plus (images tenues), ni moins");
    for it in &items {
        let (p, f) = it.frame;
        let lu = image::open(dir.join(format!("{}.png", it.name))).expect("PNG").to_rgba8();
        assert_eq!(lu.dimensions(), (64, 36));
        let attendu: Vec<u8> = render::camera_image(&doc, &render::panel_plan_at(&doc, p, f, None), &doc.panel_camera(p, f)).iter().flat_map(|c| unpremultiply(*c)).collect();
        assert!(lu.as_raw() == &attendu, "{} : l'image composée n'est pas celle du film", it.name);
        // (Les clés 2 et 4 superposent les deux calques, dont les bandes se complètent : plus de vide ; les autres ont de la transparence.)
        let deux_calques = it.name.ends_with("PL001_K02") || it.name.ends_with("PL001_K04");
        assert!(lu.pixels().any(|q| q.0[3] != 0) && (deux_calques || lu.pixels().any(|q| q.0[3] == 0)), "{} : du dessin et de la transparence", it.name);
    }
    // La clé 2 (image 2) montre les deux calques, la clé 1 (image 0) seulement « Perso » : des images différentes.
    let k1 = image::open(dir.join("SQ01_PL001_K01.png")).unwrap().to_rgba8();
    let k2 = image::open(dir.join("SQ01_PL001_K02.png")).unwrap().to_rgba8();
    assert!(k1.as_raw() != k2.as_raw());
    assert!(std::fs::read_dir(&dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".part")));
    assert_eq!(doc.panels.iter().map(|p| p.layers.clone()).collect::<Vec<_>>(), avant, "le projet n'a pas bougé");
    std::fs::remove_dir_all(&dir).ok();
    // Avec le papier, en JPEG : même nombre de fichiers, extension .jpg.
    let dir = dossier_neuf("cles-jpeg");
    let s = StillsSettings { per: StillsPer::Cle, format: StillFormat::Jpeg, ..StillsSettings::default() };
    let mut job = exportcles::job(&dir, &items, doc.frame, &s, false);
    court(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert_eq!(fichiers(&dir).len(), 5);
    assert!(fichiers(&dir).iter().all(|n| n.ends_with(".jpg")));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_per_layer_key_export_writes_each_layer_alone_on_a_transparent_background() {
    monde::lire();
    let doc = projet_cles();
    let items = exportcles::items_calques(&doc, &[0, 1], false);
    let dir = dossier_neuf("cles-calques");
    // Une taille choisie : la moitié.
    let s = StillsSettings { per: StillsPer::Cle, par_calque: true, size: crate::exportvideo::VideoSize::Half, ..StillsSettings::default() };
    let mut job = exportcles::job_calques(&dir, items.clone(), doc.frame, &s, false);
    court(&mut job, &doc);
    assert!(matches!(&job.outcome, Some(Ok(m)) if m.contains("5 images de calque")), "{:?}", job.outcome);
    assert_eq!(fichiers(&dir), ["SQ01_PL001_K01_Perso.png", "SQ01_PL001_K02_Fond.png", "SQ01_PL001_K03_Perso.png", "SQ01_PL001_K04_Fond.png", "SQ01_PL002_K01_Fond.png"], "un fichier par calque qui commence un dessin, pas de calque caché ni d'annotation ni de vidéo");
    let [w, h] = crate::exportstills::still_size(crate::exportvideo::VideoSize::Half, doc.frame);
    for it in &items {
        let (p, f) = it.frame;
        let lu = image::open(dir.join(format!("{}.png", it.name))).expect("PNG").to_rgba8();
        assert_eq!(lu.dimensions(), (w as u32, h as u32), "{}", it.name);
        // Le calque seul : exactement ce que l'export des calques séparés calcule pour lui à cette image, à la taille choisie.
        let plans = render::panel_layer_plans(&doc, p, f, false);
        let (_, plan) = plans.iter().find(|(l, _)| *l == it.layer).expect("le calque a quelque chose à cette image");
        let px = render::camera_image(&doc, plan, &doc.panel_camera(p, f));
        let octets = crate::exportstills::encode_still(&px, doc.frame, [w, h], &StillsSettings { format: StillFormat::Png, ..s }).unwrap();
        let attendu = image::load_from_memory(&octets).unwrap().to_rgba8();
        assert!(lu.as_raw() == attendu.as_raw(), "{} : pas le calque seul", it.name);
        assert!(lu.pixels().any(|q| q.0[3] != 0), "{} : du dessin", it.name);
    }
    // Sans réduction : le calque « Perso » seul ne contient pas le vert du fond.
    let dir2 = dossier_neuf("cles-calques-pleins");
    let s = StillsSettings { per: StillsPer::Cle, par_calque: true, ..StillsSettings::default() };
    let mut job = exportcles::job_calques(&dir2, items, doc.frame, &s, false);
    court(&mut job, &doc);
    let perso = image::open(dir2.join("SQ01_PL001_K03_Perso.png")).unwrap().to_rgba8();
    assert!(perso.pixels().any(|q| q.0[3] == 0), "fond transparent");
    assert!(perso.pixels().filter(|q| q.0[3] != 0).all(|q| q.0[..3] == [40, 40, 200]), "seulement le bleu du deuxième dessin de « Perso »");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&dir2).ok();
}

#[test]
fn cancelling_a_key_export_leaves_nothing_and_a_failure_keeps_a_message() {
    monde::lire();
    let mut doc = projet_cles();
    let (a, b) = (dessin(&mut doc, Color32::RED), dessin(&mut doc, Color32::BLUE));
    doc.panels[0].layers[1].frames = (0..60).map(|k| Some(if k % 2 == 0 { a } else { b })).collect();
    doc.panels[0].length = 60;
    for par_calque in [false, true] {
        let dir = dossier_neuf(&format!("cles-annule-{par_calque}"));
        let s = StillsSettings { per: StillsPer::Cle, par_calque, ..StillsSettings::default() };
        let mut job = if par_calque { exportcles::job_calques(&dir, exportcles::items_calques(&doc, &[0, 1], false), doc.frame, &s, false) } else { exportcles::job(&dir, &exportcles::items(&doc, &[0, 1], false), doc.frame, &s, false) };
        let t = Instant::now();
        while job.outcome.is_none() && !matches!(job.progress(), export::Progress::Running(n, _) if n >= 3) {
            job.step(&doc, Duration::from_millis(8));
            std::thread::sleep(Duration::from_millis(1));
            assert!(t.elapsed() < Duration::from_secs(120));
        }
        job.cancel();
        assert!(matches!(job.outcome, Some(Err(_))));
        assert!(fichiers(&dir).is_empty(), "rien ne reste après « Arrêter » (par calque : {par_calque}) : {:?}", fichiers(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }
    // Un dossier qui n'existe pas : l'export échoue avec un message, rien de trompeur ne reste.
    let rien = dossier_neuf("cles-echec").join("absent").join("encore");
    let mut job = exportcles::job_calques(&rien, exportcles::items_calques(&doc, &[1], false), doc.frame, &StillsSettings::default(), false);
    court(&mut job, &doc);
    assert!(matches!(&job.outcome, Some(Err(e)) if !e.is_empty()), "{:?}", job.outcome);
    assert!(!rien.exists());
}

// ------------------------------------------------------------------------------------------- le son

fn lire_wav(octets: &[u8]) -> (u16, Vec<i32>) {
    assert_eq!(&octets[..4], b"RIFF");
    let bits = u16::from_le_bytes([octets[34], octets[35]]);
    assert_eq!(u32::from_le_bytes(octets[24..28].try_into().unwrap()), 48_000);
    assert_eq!(u16::from_le_bytes([octets[22], octets[23]]), 2);
    let data = &octets[44..];
    assert_eq!(u32::from_le_bytes(octets[40..44].try_into().unwrap()) as usize, data.len());
    let n = bits as usize / 8;
    (bits, data.chunks(n).map(|c| if n == 2 { i16::from_le_bytes([c[0], c[1]]) as i32 } else { i32::from_le_bytes([c[0], c[1], c[2], if c[2] & 0x80 != 0 { 0xFF } else { 0 }]) }).collect())
}

fn ecrire_tous(doc: &Document, nom: &str, format: AudioFormat) -> (PathBuf, Vec<exportsonplan::SonDePlan>) {
    let plans: Vec<usize> = (0..doc.panels.len()).collect();
    let files = exportsonplan::plan(doc, &plans, format);
    let dir = dossier_neuf(nom);
    let (d, f2) = (dir.clone(), files.clone());
    let mut job = ExportJob::task("Son par plan".into(), &dir, "ok".into(), move |snap, cancel| exportsonplan::write_all(snap, &d, &f2, format, cancel).map(|_| ()));
    court(&mut job, doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    (dir, files)
}

#[test]
fn sound_per_plan_files_have_the_exact_bounds_and_add_up_to_the_film_sound() {
    monde::lire();
    // 24 images par seconde : 2000 échantillons par image ; 29 : 1655,17… (les bornes se calent à l'échantillon, la somme reste exacte).
    for fps in [24u32, 29] {
        let doc = film_with_sound(fps);
        let (dir, files) = ecrire_tous(&doc, &format!("son-plans-{fps}"), AudioFormat::default());
        assert_eq!(files.len(), 4, "la musique tient tout le film : un fichier par plan");
        assert_eq!(files.iter().map(|f| f.file.as_str()).collect::<Vec<_>>(), ["SQ01_PL001.wav", "SQ01_PL002.wav", "SQ01_PL003.wav", "SQ02_PL004.wav"]);
        assert_eq!(fichiers(&dir), ["SQ01_PL001.wav", "SQ01_PL002.wav", "SQ01_PL003.wav", "SQ02_PL004.wav"]);
        let total = export::sample_span(fps, 0, doc.total_frames());
        let mut debut = 0u64;
        let mut bout_a_bout: Vec<i32> = Vec::new();
        for (p, f) in files.iter().enumerate() {
            let attendu = export::sample_span(fps, doc.panel_start(p), doc.panels[p].frame_count());
            assert_eq!((f.start, f.len), attendu);
            assert_eq!(f.start, debut, "les plans se suivent sans trou ni recouvrement");
            debut += f.len;
            let (bits, v) = lire_wav(&std::fs::read(dir.join(&f.file)).unwrap());
            assert_eq!(bits, 16);
            assert_eq!(v.len() as u64, f.len * 2, "{} : durée exacte à l'échantillon près", f.file);
            bout_a_bout.extend(v);
        }
        assert_eq!(debut, total.1);
        // Mis bout à bout, les fichiers sont le son du film (le même mixage, écrit d'un seul tenant).
        let entier = dossier_neuf(&format!("son-entier-{fps}")).join("film.wav");
        export::write_wav(&entier, crate::audio::mix_clips_blocking(&doc), total.0, total.1).unwrap();
        let (_, v) = lire_wav(&std::fs::read(&entier).unwrap());
        assert!(v == bout_a_bout, "fps {fps} : les plans mis bout à bout ne refont pas le son du film");
        assert!(bout_a_bout.iter().any(|x| *x != 0));
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn sound_per_plan_follows_mute_skips_silent_plans_and_writes_aiff_and_24_bits() {
    monde::lire();
    let mut doc = film_with_sound(24);
    // Musique muette : seuls les plans où la voix parle (images 6-24 et 40-60 : plans 1 et 3) ont un fichier.
    doc.audio_tracks[1].muted = true;
    let plans: Vec<usize> = (0..4).collect();
    let f = exportsonplan::plan(&doc, &plans, AudioFormat::default());
    assert_eq!(f.iter().map(|x| x.plan).collect::<Vec<_>>(), [0, 2], "les plans muets ne sont pas écrits");
    assert_eq!(exportsonplan::compter(&doc, &plans), 2);
    // Étendue : seulement le plan 3.
    assert_eq!(exportsonplan::plan(&doc, &[2], AudioFormat::default()).iter().map(|x| x.plan).collect::<Vec<_>>(), [2]);
    // AIFF 24 bits : en-tête et durée du premier plan (24 images × 2000 = 48 000 échantillons), voix à 8000/32768.
    let format = AudioFormat { container: Container::Aiff, depth: Depth::Bits24 };
    let (dir, files) = ecrire_tous(&doc, "son-plans-aiff", format);
    assert_eq!(files.iter().map(|f| f.file.as_str()).collect::<Vec<_>>(), ["SQ01_PL001.aiff", "SQ01_PL003.aiff"]);
    let o = std::fs::read(dir.join("SQ01_PL001.aiff")).unwrap();
    assert_eq!(&o[..4], b"FORM");
    assert_eq!(&o[8..12], b"AIFF");
    assert_eq!(u32::from_be_bytes(o[22..26].try_into().unwrap()), 48_000, "échantillons par canal");
    assert_eq!(u16::from_be_bytes([o[26], o[27]]), 24);
    assert_eq!(o.len(), 54 + 48_000 * 2 * 3);
    // Le silence avant la voix (6 images = 12 000 échantillons), puis la voix.
    let un = |k: usize| i32::from_be_bytes([if o[54 + 3 * k] & 0x80 != 0 { 0xFF } else { 0 }, o[54 + 3 * k], o[54 + 3 * k + 1], o[54 + 3 * k + 2]]);
    assert_eq!(un(2 * 11_999), 0);
    assert!((un(2 * 12_000) as f64 - 8000.0 / 32768.0 * 8_388_607.0).abs() < 2.0, "{}", un(2 * 12_000));
    std::fs::remove_dir_all(&dir).ok();
    // WAV 24 bits.
    let (dir, _) = ecrire_tous(&doc, "son-plans-w24", AudioFormat { container: Container::Wav, depth: Depth::Bits24 });
    let (bits, v) = lire_wav(&std::fs::read(dir.join("SQ01_PL003.wav")).unwrap());
    assert_eq!((bits, v.len()), (24, 36 * 2000 * 2));
    std::fs::remove_dir_all(&dir).ok();
    // Le dernier plan (images 72-96) n'a ni voix ni musique : pas de fichier.
    assert!(!exportsonplan::plan(&doc, &[3], AudioFormat::default()).iter().any(|x| x.plan == 3));
}

#[test]
fn cancelling_or_failing_the_plan_sound_export_leaves_nothing() {
    monde::lire();
    let doc = film_with_sound(24);
    let plans: Vec<usize> = (0..4).collect();
    let files = exportsonplan::plan(&doc, &plans, AudioFormat::default());
    let dir = dossier_neuf("son-plans-annule");
    let flag = AtomicBool::new(true);
    assert!(exportsonplan::write_all(&doc, &dir, &files, AudioFormat::default(), &flag).is_err());
    assert!(fichiers(&dir).is_empty(), "annulé : aucun fichier");
    // Un dossier introuvable : l'échec est dit avec le chemin, rien ne reste.
    let absent = dir.join("absent");
    let flag = AtomicBool::new(false);
    let e = exportsonplan::write_all(&doc, &absent, &files, AudioFormat::default(), &flag).unwrap_err();
    assert!(e.contains("absent"), "{e}");
    assert!(fichiers(&dir).is_empty());
    std::fs::remove_dir_all(&dir).ok();
    // Sans le moindre son : rien à écrire.
    assert!(exportsonplan::plan(&crate::exports_v1_tests::projet_uni(2, 4, Color32::WHITE), &[0, 1], AudioFormat::default()).is_empty());
}
