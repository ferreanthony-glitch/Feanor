//! Exports pour le montage (`editorial.rs`, `editorialjob.rs`) : on relit les fichiers produits et on
//! vérifie ce qu'ils disent, image par image (structure, entrées et sorties, time codes à 24 / 25 / 30
//! et en NTSC, noms, chemins relatifs, pistes son). Aucun de ces formats n'est ouvert dans un vrai
//! logiciel de montage ici : voir `docs/EXPORT_MONTAGE.md`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;

use crate::app::App;
use crate::audio::Pcm;
use crate::document::Document;
use crate::editorial::{self, Media, Options, Plan, Rate};
use crate::editorialjob::{self, EditorialJob, Stage};
use crate::export;
use crate::transitions::{Align, Kind};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-montage-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Quatre plans de 24, 12, 36 et 24 images, les deux premiers dans la séquence 1, le troisième aussi, le
/// dernier dans la séquence 2. Numérotation verrouillée.
fn film(fps: u32) -> Document {
    let mut doc = Document::new(64, 36, fps);
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

fn opts() -> Options {
    Options { sound: false, ..Options::default() }
}

fn plan_of(doc: &Document, o: &Options) -> Plan {
    editorial::build_plan(doc, o, 0, "Mon film").expect("plan")
}

fn dissolve(doc: &mut Document, cut: usize, frames: usize, align: Align) {
    let id = doc.set_transition(cut, Kind::Dissolve).expect("fondu enchaîné");
    let t = doc.transitions.iter_mut().find(|t| t.id == id).unwrap();
    t.frames = frames;
    t.align = align;
}

// ------------------------------------------------------------------------------------ time codes

#[test]
fn timecodes_read_and_write_at_24_25_30() {
    for (fps, start) in [(24u32, "01:00:00:00"), (25, "01:00:00:00"), (30, "01:00:00:00"), (24, "00:59:58:23"), (25, "10:00:00:24"), (30, "00:00:00:29")] {
        let n = editorial::parse_timecode(start, fps).unwrap_or_else(|| panic!("{start} à {fps}"));
        assert_eq!(editorial::timecode(n, fps), start);
    }
    assert_eq!(editorial::parse_timecode("01:00:00:00", 24), Some(86_400));
    assert_eq!(editorial::parse_timecode("01:00:00:00", 25), Some(90_000));
    assert_eq!(editorial::parse_timecode("01:00:00:00", 30), Some(108_000));
    assert_eq!(editorial::timecode(86_400 + 25, 24), "01:00:01:01");
    // L'image 24 n'existe pas à 24 i/s ; les minutes et secondes vont jusqu'à 59.
    for bad in ["01:00:00:24", "01:60:00:00", "01:00:60:00", "1:00:00", "aa:bb:cc:dd", "", "01:00:00:001"] {
        assert!(editorial::parse_timecode(bad, 24).is_none(), "{bad}");
    }
    assert_eq!(editorial::parse_timecode("01;00;00;12", 24), Some(86_412), "les points-virgules (saut d'images) sont lus comme des deux-points");
}

#[test]
fn ntsc_is_the_real_rate_times_1000_over_1001_and_only_for_24_30_60() {
    let r = Rate::new(24, true);
    assert!(r.ntsc && (r.real() - 23.976_023_976).abs() < 1e-6);
    assert_eq!(r.ffmpeg(), "24000/1001");
    assert_eq!(r.label(), "23,976 i/s (NTSC)");
    assert_eq!(Rate::new(30, true).ffmpeg(), "30000/1001");
    assert_eq!(Rate::new(60, true).ffmpeg(), "60000/1001");
    assert!(!Rate::new(25, true).ntsc, "pas de NTSC à 25 i/s");
    assert!(!Rate::new(12, true).ntsc);
    assert_eq!(Rate::new(24, false).ffmpeg(), "24");
    assert_eq!(Rate::new(24, false).label(), "24 i/s");
    // Le time code garde les numéros d'images du projet.
    assert_eq!(editorial::timecode(86_400, 24), "01:00:00:00");
}

#[test]
fn names_are_made_safe() {
    assert_eq!(editorial::file_safe("Écran noir / n°2"), "Ecran_noir___n_2");
    assert_eq!(editorial::file_safe("  "), "sans_nom");
    assert_eq!(editorial::file_safe("SQ01_PL012A"), "SQ01_PL012A");
    assert_eq!(editorial::ascii_fold("Évasion à Paris — œuvre"), "Evasion a Paris _ ouvre");
}

// ------------------------------------------------------------------------------------ le plan d'export

#[test]
fn the_plan_has_one_clip_per_plan_with_exact_positions_and_stable_names() {
    let mut doc = film(24);
    // Un plan inséré entre le 2 et le 3 : 2A (numérotation verrouillée).
    let i = doc.add_scene_after(1);
    doc.panels[i].set_length(10);
    let plan = plan_of(&doc, &opts());
    let names: Vec<&str> = plan.clips.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["SQ01_PL001", "SQ01_PL002", "SQ01_PL002A", "SQ01_PL003", "SQ02_PL004"]);
    let starts: Vec<usize> = plan.clips.iter().map(|c| c.start).collect();
    let lens: Vec<usize> = plan.clips.iter().map(|c| c.len).collect();
    assert_eq!(lens, [24, 12, 10, 36, 24]);
    assert_eq!(starts, [0, 24, 36, 46, 82]);
    assert_eq!(plan.total, 106);
    assert!(plan.clips.iter().all(|c| c.head == 0 && c.tail == 0), "sans fondu, pas de poignée");
    assert_eq!(plan.clips[2].file, "media/SQ01_PL002A.mov");
    assert!(plan.clips.iter().all(|c| !c.file.contains('\\') && !c.file.starts_with('/')), "chemins relatifs, avec des /");
    // Les noms ne changent pas quand un plan s'ajoute plus loin.
    let before = plan.clips[0].name.clone();
    doc.add_scene_after(4);
    assert_eq!(plan_of(&doc, &opts()).clips[0].name, before);
    // Les bobines de l'EDL : 8 caractères au plus, sans doublon.
    let reels: Vec<&str> = plan.clips.iter().map(|c| c.reel.as_str()).collect();
    assert!(reels.iter().all(|r| r.len() <= 8), "{reels:?}");
    assert_eq!(reels.iter().collect::<std::collections::HashSet<_>>().len(), reels.len());
}

#[test]
fn a_dotted_plan_number_makes_a_safe_file_name() {
    let mut doc = film(24);
    doc.add_scene_after(0); // 1A
    doc.add_scene_after(0); // 1.5
    let plan = plan_of(&doc, &opts());
    let c = plan.clips.iter().find(|c| c.name.ends_with("PL001.5")).expect("plan 1.5");
    assert_eq!(c.file, "media/SQ01_PL001_5.mov", "le point devient _ dans le nom de fichier, pas dans le nom du clip");
    assert!(c.reel.len() <= 8 && !c.reel.contains('.'));
}

#[test]
fn the_current_sequence_only_starts_at_zero() {
    let doc = film(24);
    let o = Options { whole_film: false, ..opts() };
    let plan = editorial::build_plan(&doc, &o, 3, "Séquence 2").unwrap();
    assert_eq!(plan.clips.len(), 1);
    assert_eq!((plan.clips[0].name.as_str(), plan.clips[0].start, plan.total), ("SQ02_PL004", 0, 24));
    let plan = editorial::build_plan(&doc, &o, 0, "Séquence 1").unwrap();
    assert_eq!(plan.clips.iter().map(|c| c.len).sum::<usize>(), 72);
    assert_eq!(plan.clips.len(), 3);
}

#[test]
fn a_bad_start_timecode_or_an_empty_film_is_an_error_in_french() {
    let doc = film(24);
    let e = editorial::build_plan(&doc, &Options { start_tc: "25:61:00:99".into(), ..opts() }, 0, "x").err().expect("erreur");
    assert!(e.contains("Time code de départ illisible") && e.contains("0 à 23"), "{e}");
}

// ------------------------------------------------------------------------------------ l'EDL

#[derive(Debug, Clone)]
struct Ev {
    number: usize,
    reel: String,
    chan: String,
    trans: String,
    dur: Option<i64>,
    src_in: i64,
    src_out: i64,
    rec_in: i64,
    rec_out: i64,
    clip: Option<String>,
}

struct Edl {
    title: String,
    fcm: String,
    events: Vec<Ev>,
    comments: Vec<String>,
}

fn parse_edl(text: &str, fps: u32) -> Edl {
    let mut edl = Edl { title: String::new(), fcm: String::new(), events: Vec::new(), comments: Vec::new() };
    let tc = |s: &str| editorial::parse_timecode(s, fps).unwrap_or_else(|| panic!("time code illisible : {s}"));
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if let Some(t) = line.strip_prefix("TITLE:") {
            edl.title = t.trim().to_string();
        } else if let Some(f) = line.strip_prefix("FCM:") {
            edl.fcm = f.trim().to_string();
        } else if let Some(c) = line.strip_prefix('*') {
            if let Some(name) = c.trim().strip_prefix("FROM CLIP NAME:") {
                edl.events.last_mut().expect("un événement avant son nom").clip = Some(name.trim().to_string());
            } else {
                edl.comments.push(c.trim().to_string());
            }
        } else {
            let t: Vec<&str> = line.split_whitespace().collect();
            let (dur, rest) = match t.get(3) {
                Some(&"D") => (Some(t[4].parse::<i64>().expect("durée du fondu")), 5),
                _ => (None, 4),
            };
            assert_eq!(t.len(), rest + 4, "ligne d'événement mal formée : {line}");
            edl.events.push(Ev {
                number: t[0].parse().expect("numéro d'événement"),
                reel: t[1].to_string(),
                chan: t[2].to_string(),
                trans: t[3].to_string(),
                dur,
                src_in: tc(t[rest]),
                src_out: tc(t[rest + 1]),
                rec_in: tc(t[rest + 2]),
                rec_out: tc(t[rest + 3]),
                clip: None,
            });
        }
    }
    edl
}

#[test]
fn the_edl_of_a_film_without_transitions_is_frame_exact_at_24_25_30() {
    for fps in [24u32, 25, 30] {
        let doc = film(fps);
        let plan = plan_of(&doc, &opts());
        let text = editorial::write_edl(&plan);
        assert!(text.is_ascii(), "l'EDL est en ASCII");
        let edl = parse_edl(&text, fps);
        assert_eq!(edl.fcm, "NON-DROP FRAME");
        assert_eq!(edl.title, "Mon film");
        assert_eq!(edl.events.len(), 4);
        let start = editorial::parse_timecode("01:00:00:00", fps).unwrap();
        let mut at = start;
        for (k, (e, c)) in edl.events.iter().zip(&plan.clips).enumerate() {
            assert_eq!((e.number, e.chan.as_str(), e.trans.as_str(), e.dur), (k + 1, "V", "C", None));
            assert_eq!(e.reel, c.reel);
            assert_eq!(e.clip.as_deref(), Some(c.name.as_str()), "{fps} i/s");
            assert_eq!((e.src_in, e.src_out), (0, c.len as i64), "le fichier du plan commence à zéro");
            assert_eq!((e.rec_in, e.rec_out), (at, at + c.len as i64), "{fps} i/s : entrée et sortie exactes");
            at += c.len as i64;
        }
        assert_eq!(at, start + plan.total as i64);
    }
}

#[test]
fn the_edl_layout_is_the_cmx3600_columns() {
    let doc = film(24);
    let text = editorial::write_edl(&plan_of(&doc, &opts()));
    let first = text.lines().find(|l| l.starts_with("001")).expect("événement 1");
    assert_eq!(first, "001  PL001    V     C        00:00:00:00 00:00:01:00 01:00:00:00 01:00:01:00");
    assert!(text.contains("* FROM CLIP NAME: SQ01_PL001\n"));
}

#[test]
fn the_start_timecode_option_moves_every_event() {
    let doc = film(25);
    let o = Options { start_tc: "10:00:00:00".into(), ..opts() };
    let plan = plan_of(&doc, &o);
    let edl = parse_edl(&editorial::write_edl(&plan), 25);
    assert_eq!(edl.events[0].rec_in, editorial::parse_timecode("10:00:00:00", 25).unwrap());
    assert_eq!(edl.events[0].src_in, 0);
}

#[test]
fn a_centered_dissolve_is_a_two_line_event_with_handles() {
    let mut doc = film(24);
    dissolve(&mut doc, 1, 12, Align::Centre); // entre le plan 1 (24 images) et le plan 2 (12 images)
    let plan = plan_of(&doc, &opts());
    assert_eq!(plan.dissolves.len(), 1);
    let d = plan.dissolves[0];
    assert_eq!((d.a, d.b, d.cut, d.start, d.end), (0, 1, 24, 18, 30), "6 images avant la coupe, 6 après");
    // Les poignées : le plan sortant a 6 images tenues après lui, le plan entrant 6 avant lui.
    assert_eq!((plan.clips[0].head, plan.clips[0].tail), (0, 6));
    assert_eq!((plan.clips[1].head, plan.clips[1].tail), (6, 0));
    assert_eq!(plan.clips[0].file_frames(), 30);
    assert_eq!(plan.clips[1].file_frames(), 18);
    assert_eq!(editorialjob::clip_frames(&plan.clips[0]).len(), 30);
    assert_eq!(editorialjob::clip_frames(&plan.clips[1]).len(), 18);

    let edl = parse_edl(&editorial::write_edl(&plan), 24);
    let start = 86_400;
    // Événement 1 : le plan sortant jusqu'au début du fondu (image 18).
    let e = &edl.events;
    assert_eq!((e[0].number, e[0].trans.as_str(), e[0].src_in, e[0].src_out, e[0].rec_in, e[0].rec_out), (1, "C", 0, 18, start, start + 18));
    // Événement 2, première ligne : le même plan, à longueur nulle, au début du fondu.
    assert_eq!((e[1].number, e[1].reel.as_str(), e[1].trans.as_str(), e[1].src_in, e[1].src_out, e[1].rec_in, e[1].rec_out), (2, plan.clips[0].reel.as_str(), "C", 18, 18, start + 18, start + 18));
    // Deuxième ligne : le plan entrant en fondu de 12 images, entré dans sa poignée (6 images avant son début).
    assert_eq!((e[2].number, e[2].reel.as_str(), e[2].trans.as_str(), e[2].dur), (2, plan.clips[1].reel.as_str(), "D", Some(12)));
    assert_eq!((e[2].src_in, e[2].src_out, e[2].rec_in, e[2].rec_out), (0, 18, start + 18, start + 36));
    assert_eq!(e[2].clip.as_deref(), Some("SQ01_PL002"));
    // Les durées source et enregistrement sont égales pour chaque ligne qui joue.
    for l in [&e[0], &e[2], &e[3]] {
        assert_eq!(l.src_out - l.src_in, l.rec_out - l.rec_in);
    }
    // Le plan sortant a de quoi finir le fondu : sa sortie + 12 images tient dans son fichier.
    assert!(e[1].src_out as usize + 12 <= plan.clips[0].file_frames());
    // Rien ne manque ni ne se recouvre : le film dure 96 images.
    assert_eq!(e.last().unwrap().rec_out, start + plan.total as i64);
    assert_eq!(e.len(), 5);
}

#[test]
fn a_dissolve_aligned_before_or_after_the_cut_has_one_sided_handles() {
    // Le fondu entre le plan 2 (12 images) et le plan 3 (36 images), coupe à l'image 36.
    for (align, head, tail, zone) in [(Align::Before, 12, 0, (24, 36)), (Align::After, 0, 12, (36, 48))] {
        let mut doc = film(24);
        dissolve(&mut doc, 2, 12, align);
        let plan = plan_of(&doc, &opts());
        let d = plan.dissolves[0];
        assert_eq!(d.cut, 36);
        assert_eq!((d.start, d.end), zone);
        // « Finit à la coupe » : le plan entrant a 12 images de poignée avant lui ; « commence à la coupe » :
        // le plan sortant en a 12 après lui.
        assert_eq!((plan.clips[1].tail, plan.clips[2].head), (tail, head));
    }
}

#[test]
fn a_fade_from_and_to_black_is_a_dissolve_with_the_black_reel() {
    let mut doc = film(24);
    doc.set_transition(0, Kind::Black).expect("fondu depuis le noir");
    let n = doc.panels.len();
    doc.set_transition(n, Kind::Black).expect("fondu vers le noir");
    for t in &mut doc.transitions {
        t.frames = 8;
    }
    let plan = plan_of(&doc, &opts());
    assert_eq!(plan.fades.len(), 2);
    let edl = parse_edl(&editorial::write_edl(&plan), 24);
    let e = &edl.events;
    let start = 86_400;
    // Début : le noir à longueur nulle, puis le premier plan en fondu de 8 images.
    assert_eq!((e[0].reel.as_str(), e[0].trans.as_str(), e[0].rec_in, e[0].rec_out), ("BL", "C", start, start));
    assert_eq!((e[1].number, e[1].trans.as_str(), e[1].dur, e[1].src_in, e[1].rec_in), (1, "D", Some(8), 0, start));
    // Fin : le dernier plan, à longueur nulle, puis le noir en fondu de 8 images.
    let n = e.len();
    assert_eq!((e[n - 1].reel.as_str(), e[n - 1].trans.as_str(), e[n - 1].dur), ("BL", "D", Some(8)));
    assert_eq!((e[n - 1].rec_in, e[n - 1].rec_out), (start + plan.total as i64 - 8, start + plan.total as i64));
    assert_eq!((e[n - 2].rec_in, e[n - 2].rec_out), (start + plan.total as i64 - 8, start + plan.total as i64 - 8));
    // Pour le XML et l'OTIO : une coupe, dite.
    assert!(plan.skipped.iter().all(|s| s.lost == [false, true, true]), "{:?}", plan.skipped);
    assert_eq!(plan.skipped.len(), 2);
}

#[test]
fn transitions_without_a_description_in_the_formats_are_written_as_cuts_and_said() {
    let mut doc = film(24);
    doc.set_transition(1, Kind::Wipe).expect("volet");
    doc.set_transition(2, Kind::White).expect("fondu au blanc");
    let plan = plan_of(&doc, &opts());
    assert!(plan.dissolves.is_empty() && plan.fades.is_empty());
    assert_eq!(plan.skipped.len(), 2);
    assert!(plan.clips.iter().all(|c| c.head == 0 && c.tail == 0), "pas de poignée pour une coupe");
    let edl_text = editorial::write_edl(&plan);
    let edl = parse_edl(&edl_text, 24);
    assert!(edl.events.iter().all(|e| e.trans == "C"));
    assert!(edl.comments.iter().any(|c| c.contains("Volet") && c.contains("coupe")), "{:?}", edl.comments);
    let readme = editorial::readme(&plan, &opts());
    assert!(readme.contains("Volet") && readme.contains("Fondu au blanc") && readme.contains("vérifier par Anthony"), "{readme}");
}

#[test]
fn ntsc_changes_the_rate_flags_not_the_frame_numbers() {
    let doc = film(24);
    let o = Options { ntsc: true, ..opts() };
    let plan = plan_of(&doc, &o);
    assert!(plan.rate.ntsc);
    let edl = parse_edl(&editorial::write_edl(&plan), 24);
    assert_eq!(edl.fcm, "NON-DROP FRAME");
    assert_eq!(edl.events[0].rec_in, 86_400, "les numéros d'images sont ceux du projet");
    assert!(edl.comments.iter().any(|c| c.contains("NTSC")));
    let xml = crate::xml::parse(&editorial::write_xml(&plan, None)).expect("XML lisible");
    assert_eq!(xml.path("sequence/rate/timebase").unwrap().text, "24");
    assert_eq!(xml.path("sequence/rate/ntsc").unwrap().text, "TRUE");
    let otio: serde_json::Value = serde_json::from_str(&editorial::write_otio(&plan, None)).unwrap();
    let rate = otio["tracks"]["children"][0]["children"][0]["source_range"]["duration"]["rate"].as_f64().unwrap();
    assert!((rate - 24_000.0 / 1001.0).abs() < 1e-9, "{rate}");
    assert_eq!(otio["global_start_time"]["value"].as_f64().unwrap(), 86_400.0);
    // 29,97 : seulement pour 30 i/s.
    assert!(!Rate::new(25, true).ntsc);
}

// ------------------------------------------------------------------------------------ le XML

fn xml_of(plan: &Plan, abs: Option<&Path>) -> crate::xml::Element {
    crate::xml::parse(&editorial::write_xml(plan, abs)).expect("le XML se relit")
}

fn num(e: &crate::xml::Element, path: &str) -> i64 {
    e.path(path).unwrap_or_else(|| panic!("pas de <{path}>")).text.trim().parse().unwrap_or_else(|_| panic!("<{path}> n'est pas un nombre"))
}

#[test]
fn the_xml_has_the_xmeml_structure_with_exact_in_and_out() {
    let mut doc = film(24);
    dissolve(&mut doc, 1, 12, Align::Centre);
    doc.properties.title = "Tom & Jerry <1>".into();
    let plan = editorial::build_plan(&doc, &opts(), 0, &doc.properties.title.clone()).unwrap();
    let text = editorial::write_xml(&plan, None);
    assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n<xmeml version=\"4\">"), "{}", &text[..120]);
    let root = crate::xml::parse(&text).expect("XML lisible");
    assert_eq!(root.name, "xmeml");
    assert_eq!(root.attr("version"), Some("4"));
    let seq = root.child("sequence").expect("sequence");
    assert_eq!(seq.child("name").unwrap().text, "Tom & Jerry <1>", "les caractères spéciaux sont protégés puis relus");
    assert_eq!(num(seq, "duration"), 96);
    assert_eq!(num(seq, "rate/timebase"), 24);
    assert_eq!(seq.path("rate/ntsc").unwrap().text, "FALSE");
    assert_eq!(seq.path("timecode/string").unwrap().text, "01:00:00:00");
    assert_eq!(num(seq, "timecode/frame"), 86_400);
    assert_eq!(seq.path("timecode/displayformat").unwrap().text, "NDF");
    assert_eq!(num(seq, "media/video/format/samplecharacteristics/width"), 64);
    let track = seq.path("media/video/track").expect("piste vidéo");
    let items: Vec<&crate::xml::Element> = track.children_named("clipitem").collect();
    assert_eq!(items.len(), 4);
    let mut expected_start = 0;
    for (item, clip) in items.iter().zip(&plan.clips) {
        assert_eq!(item.child("name").unwrap().text, clip.name);
        let (start, end, i, o) = (num(item, "start"), num(item, "end"), num(item, "in"), num(item, "out"));
        assert_eq!((start, end), (clip.start as i64, clip.end() as i64));
        assert_eq!(start, expected_start, "les clips se suivent sans trou");
        expected_start = end;
        assert_eq!(end - start, o - i, "durée dans la séquence = durée dans le fichier");
        assert_eq!((i, o), (clip.head as i64, (clip.head + clip.len) as i64));
        assert_eq!(num(item, "duration"), clip.file_frames() as i64, "durée du fichier, poignées comprises");
        let file = item.child("file").expect("file");
        assert_eq!(file.child("pathurl").unwrap().text, clip.file, "chemin relatif");
        assert_eq!(file.child("name").unwrap().text, clip.file.trim_start_matches("media/"));
        assert_eq!(num(file, "duration"), clip.file_frames() as i64);
        assert_eq!(file.path("timecode/string").unwrap().text, "00:00:00:00");
    }
    // Le fondu : entre le clip 1 et le clip 2, dans la même piste, de l'image 18 à 30.
    let order: Vec<&str> = track.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(order, ["clipitem", "transitionitem", "clipitem", "clipitem", "clipitem"]);
    let t = track.child("transitionitem").unwrap();
    assert_eq!((num(t, "start"), num(t, "end")), (18, 30));
    assert_eq!(t.child("alignment").unwrap().text, "center");
    assert_eq!(t.path("effect/name").unwrap().text, "Cross Dissolve");
    assert_eq!(t.path("effect/effecttype").unwrap().text, "transition");
    // Pas de son demandé : pas de bloc audio.
    assert!(seq.path("media/audio").is_none());
}

#[test]
fn xml_transition_alignment_follows_the_chosen_alignment() {
    for (align, word) in [(Align::Centre, "center"), (Align::Before, "end"), (Align::After, "start")] {
        let mut doc = film(24);
        dissolve(&mut doc, 2, 6, align);
        let root = xml_of(&plan_of(&doc, &opts()), None);
        let t = root.path("sequence/media/video/track/transitionitem").unwrap();
        assert_eq!(t.child("alignment").unwrap().text, word);
    }
}

#[test]
fn the_xml_can_carry_absolute_paths_instead() {
    let doc = film(24);
    let plan = plan_of(&doc, &opts());
    let dir = temp("absolu");
    let root = xml_of(&plan, Some(&dir));
    let url = root.path("sequence/media/video/track/clipitem/file/pathurl").unwrap().text.clone();
    assert!(url.starts_with("file://localhost/"), "{url}");
    assert!(url.ends_with("/media/SQ01_PL001.mov"), "{url}");
    assert!(!url.contains('\\') && !url.contains(' '), "{url}");
    assert_eq!(editorial::file_url(Path::new(r"C:\Mes films\é\x.mov")), "file://localhost/C:/Mes%20films/%C3%A9/x.mov");
}

// ------------------------------------------------------------------------------------ le son

fn tone(frames: usize, level: i16) -> Arc<Pcm> {
    Arc::new(Pcm { data: vec![level; frames * 2], peaks: Vec::new() })
}

/// Un film de quatre plans avec deux pistes son : « Voix » (deux extraits) et « Musique ».
fn film_with_sound(fps: u32) -> Document {
    let mut doc = film(fps);
    let voix = doc.add_sound("voix.wav".into(), "wav".into(), Arc::new(Vec::new()), tone(48_000 * 4, 8_000));
    let zik = doc.add_sound("musique.wav".into(), "wav".into(), Arc::new(Vec::new()), tone(48_000 * 6, 4_000));
    doc.add_audio_track();
    doc.add_audio_track();
    doc.audio_tracks[0].name = "Voix é".into();
    doc.audio_tracks[1].name = "Musique".into();
    doc.audio_tracks[1].volume = 0.5;
    doc.add_clip(0, voix, 6, 0, 18).expect("extrait 1");
    doc.add_clip(0, voix, 40, 12, 20).expect("extrait 2");
    doc.add_clip(1, zik, 0, 0, 96).expect("extrait 3");
    doc
}

#[test]
fn sound_clips_become_files_and_stereo_track_pairs_in_the_xml() {
    let doc = film_with_sound(24);
    let plan = editorial::build_plan(&doc, &Options { sound: true, ..Options::default() }, 0, "Son").unwrap();
    assert_eq!(plan.audio.len(), 3);
    let a: Vec<(usize, &str, usize, usize)> = plan.audio.iter().map(|a| (a.track, a.file.as_str(), a.start, a.len)).collect();
    assert_eq!(a, [(1, "audio/A1_Voix_e_01.wav", 6, 18), (1, "audio/A1_Voix_e_02.wav", 40, 20), (2, "audio/A2_Musique_01.wav", 0, 96)]);
    // Le fichier : en-tête WAV exact, durée exacte, volume de la piste appliqué.
    let bytes = editorial::wav_bytes(&plan.audio_sources[2]);
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 48_000);
    assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 2, "stéréo");
    let samples = (bytes.len() - 44) / 4;
    assert_eq!(samples, 96 * 2000, "96 images à 24 i/s = 192 000 échantillons à 48 kHz");
    let first = i16::from_le_bytes(bytes[44..46].try_into().unwrap());
    assert!((first as i32 - 2_000).abs() <= 2, "musique à 4 000 × volume 0,5 : {first}");

    let root = xml_of(&plan, None);
    let audio = root.path("sequence/media/audio").expect("bloc audio");
    assert_eq!(num(audio, "numOutputChannels"), 2);
    let tracks: Vec<&crate::xml::Element> = audio.children_named("track").collect();
    assert_eq!(tracks.len(), 4, "deux pistes de SHOT × deux voies");
    for (n, t) in tracks.iter().enumerate() {
        let items: Vec<&crate::xml::Element> = t.children_named("clipitem").collect();
        assert_eq!(items.len(), if n < 2 { 2 } else { 1 });
        for item in &items {
            assert_eq!(num(item, "end") - num(item, "start"), num(item, "out") - num(item, "in"));
            assert_eq!(item.path("sourcetrack/mediatype").unwrap().text, "audio");
            assert_eq!(num(item, "sourcetrack/trackindex"), if n % 2 == 0 { 1 } else { 2 });
        }
    }
    let first = tracks[0].child("clipitem").unwrap();
    assert_eq!((num(first, "start"), num(first, "end")), (6, 24));
    let file = first.child("file").unwrap();
    assert_eq!(file.child("pathurl").unwrap().text, "audio/A1_Voix_e_01.wav");
    assert_eq!(num(file, "media/audio/channelcount"), 2);
    assert_eq!(num(file, "media/audio/samplecharacteristics/samplerate"), 48_000);
    // La voie droite pointe le même fichier (référence par identifiant).
    let right = tracks[1].child("clipitem").unwrap().child("file").unwrap();
    assert_eq!(right.attr("id"), file.attr("id"));
    assert!(right.children.is_empty());
}

#[test]
fn sound_clips_become_audio_tracks_with_gaps_in_the_otio() {
    let doc = film_with_sound(24);
    let plan = editorial::build_plan(&doc, &Options { sound: true, ..Options::default() }, 0, "Son").unwrap();
    let otio: serde_json::Value = serde_json::from_str(&editorial::write_otio(&plan, None)).unwrap();
    let tracks = otio["tracks"]["children"].as_array().unwrap();
    assert_eq!(tracks.len(), 3, "une vidéo, deux sons");
    assert_eq!(tracks[1]["kind"], "Audio");
    let items = tracks[1]["children"].as_array().unwrap();
    let kinds: Vec<&str> = items.iter().map(|i| i["OTIO_SCHEMA"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["Gap.1", "Clip.1", "Gap.1", "Clip.1"]);
    let dur = |i: &serde_json::Value| i["source_range"]["duration"]["value"].as_f64().unwrap() as usize;
    assert_eq!(items.iter().map(dur).collect::<Vec<_>>(), [6, 18, 16, 20], "les vides et les extraits recouvrent exactement la piste jusqu'à 60");
    assert_eq!(items[1]["media_reference"]["target_url"], "audio/A1_Voix_e_01.wav");
    assert_eq!(tracks[2]["children"][0]["media_reference"]["available_range"]["duration"]["value"].as_f64().unwrap(), 96.0);
}

#[test]
fn muted_tracks_and_solo_are_not_written() {
    let mut doc = film_with_sound(24);
    doc.audio_tracks[1].muted = true;
    let o = Options { sound: true, ..Options::default() };
    let plan = editorial::build_plan(&doc, &o, 0, "x").unwrap();
    assert_eq!(plan.audio.len(), 2);
    assert_eq!(plan.silent_tracks, ["Musique"]);
    assert!(editorial::readme(&plan, &o).contains("Pistes son pas écrites"));
    doc.audio_tracks[1].muted = false;
    doc.audio_tracks[0].solo = true;
    let plan = editorial::build_plan(&doc, &o, 0, "x").unwrap();
    assert_eq!(plan.audio.len(), 2, "une piste en solo : seules les pistes en solo");
}

#[test]
fn sound_is_cut_to_the_exported_sequence() {
    let doc = film_with_sound(24);
    // Séquence 2 = image 72 à 96 ; la musique (0 à 96) y entre à 72, la voix n'y est pas.
    let o = Options { sound: true, whole_film: false, ..Options::default() };
    let plan = editorial::build_plan(&doc, &o, 3, "Séquence 2").unwrap();
    assert_eq!(plan.audio.len(), 1);
    let a = &plan.audio[0];
    assert_eq!((a.start, a.len), (0, 24));
    let src = &plan.audio_sources[0];
    assert_eq!((src.clip.start, src.clip.offset, src.clip.length), (72, 72, 24), "l'extrait commence 72 images plus loin dans son son");
}

// ------------------------------------------------------------------------------------ l'OTIO

#[test]
fn the_otio_is_a_timeline_with_exact_ranges_transitions_and_relative_urls() {
    let mut doc = film(25);
    dissolve(&mut doc, 1, 12, Align::Centre);
    let plan = plan_of(&doc, &Options { start_tc: "01:00:00:00".into(), ..opts() });
    let otio: serde_json::Value = serde_json::from_str(&editorial::write_otio(&plan, None)).expect("JSON");
    assert_eq!(otio["OTIO_SCHEMA"], "Timeline.1");
    assert_eq!(otio["name"], "Mon film");
    assert_eq!(otio["global_start_time"], serde_json::json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": 25.0, "value": 90_000.0 }));
    assert_eq!(otio["tracks"]["OTIO_SCHEMA"], "Stack.1");
    let v = &otio["tracks"]["children"][0];
    assert_eq!((v["OTIO_SCHEMA"].as_str(), v["kind"].as_str()), (Some("Track.1"), Some("Video")));
    let items = v["children"].as_array().unwrap();
    let kinds: Vec<&str> = items.iter().map(|i| i["OTIO_SCHEMA"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["Clip.1", "Transition.1", "Clip.1", "Clip.1", "Clip.1"]);
    let val = |x: &serde_json::Value| x["value"].as_f64().unwrap();
    let mut clip = 0;
    for item in items.iter().filter(|i| i["OTIO_SCHEMA"] == "Clip.1") {
        let c = &plan.clips[clip];
        clip += 1;
        assert_eq!(item["name"], c.name.as_str());
        assert_eq!(item["source_range"]["OTIO_SCHEMA"], "TimeRange.1");
        assert_eq!(val(&item["source_range"]["start_time"]), c.head as f64, "entrée dans le fichier");
        assert_eq!(val(&item["source_range"]["duration"]), c.len as f64);
        assert_eq!(item["source_range"]["start_time"]["rate"].as_f64(), Some(25.0));
        let r = &item["media_reference"];
        assert_eq!(r["OTIO_SCHEMA"], "ExternalReference.1");
        assert_eq!(r["target_url"], c.file.as_str());
        assert_eq!(val(&r["available_range"]["duration"]), c.file_frames() as f64);
        assert_eq!(val(&r["available_range"]["start_time"]), 0.0);
    }
    let t = &items[1];
    assert_eq!(t["transition_type"], "SMPTE_Dissolve");
    assert_eq!((val(&t["in_offset"]), val(&t["out_offset"])), (6.0, 6.0), "6 images avant la coupe, 6 après");
    // Les clips couvrent la durée totale.
    let total: f64 = items.iter().filter(|i| i["OTIO_SCHEMA"] == "Clip.1").map(|i| val(&i["source_range"]["duration"])).sum();
    assert_eq!(total, plan.total as f64);
}

// ------------------------------------------------------------------------------------ le texte d'accompagnement

#[test]
fn the_readme_says_what_was_written_and_what_to_check() {
    let mut doc = film_with_sound(24);
    dissolve(&mut doc, 1, 12, Align::Centre);
    let o = Options { sound: true, ..Options::default() };
    let plan = editorial::build_plan(&doc, &o, 0, "Mon film").unwrap();
    let r = editorial::readme(&plan, &o);
    for needle in ["*.edl", "*.xml", "*.otio", "media/", "audio/", "Poignées", "relatifs", "SQ01_PL012A", "vérifier par Anthony", "Premiere Pro", "DaVinci Resolve", "Avid", "01:00:00:00"] {
        assert!(r.contains(needle), "« {needle} » manque :\n{r}");
    }
}

// ------------------------------------------------------------------------------------ la tâche

fn ffmpeg_and_encoders() -> Option<(PathBuf, export::Encoders)> {
    let ffmpeg = export::find_ffmpeg(None)?;
    let enc = export::probe(&ffmpeg).ok()?;
    Some((ffmpeg, enc))
}

/// Les images d'un fichier vidéo, comptées par ffmpeg (copie vers rien).
fn count_frames(ffmpeg: &Path, file: &Path) -> usize {
    let out = std::process::Command::new(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-i"])
        .arg(file)
        .args(["-map", "0:v:0", "-c", "copy", "-f", "null", "-"])
        .output()
        .expect("ffmpeg");
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    let at = text.rfind("frame=").unwrap_or_else(|| panic!("pas de compte d'images dans :\n{text}"));
    text[at + 6..].trim_start().chars().take_while(char::is_ascii_digit).collect::<String>().parse().expect("nombre d'images")
}

fn run(job: &mut EditorialJob, doc: &Document) {
    let t = std::time::Instant::now();
    while job.running() {
        job.step(doc, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(120), "export trop long");
    }
}

#[test]
fn a_real_export_writes_every_file_with_the_right_number_of_frames() {
    let Some((ffmpeg, enc)) = ffmpeg_and_encoders() else {
        eprintln!("ffmpeg absent : export réel sauté");
        return;
    };
    let media = if enc.prores { Media::ProRes } else if enc.h264.is_some() { Media::H264 } else { return };
    let mut doc = film_with_sound(24);
    dissolve(&mut doc, 1, 12, Align::Centre);
    let o = Options { sound: true, media, ..Options::default() };
    let plan = editorial::build_plan(&doc, &o, 0, "Mon film é").unwrap();
    let parent = temp("reel");
    let mut job = EditorialJob::start(plan.clone(), o.clone(), ffmpeg.clone(), enc, &parent).expect("démarré");
    assert_eq!(job.dir, parent.join("Mon_film_e_montage"));
    run(&mut job, &doc);
    let report = match job.outcome.clone().expect("terminé") {
        Ok(r) => r,
        Err(e) => panic!("{e}"),
    };
    assert_eq!(report.plans, 4);
    assert_eq!(report.sounds, 3);
    // Les fichiers de montage.
    let names: Vec<String> = report.files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["Mon_film_e.edl", "Mon_film_e.xml", "Mon_film_e.otio", "LISEZ-MOI.txt"]);
    let dir = &report.dir;
    // Chaque média cité existe, avec le bon nombre d'images (poignées comprises).
    for c in &plan.clips {
        let f = dir.join(&c.file);
        assert!(f.is_file(), "{} manque", f.display());
        assert_eq!(count_frames(&ffmpeg, &f), c.file_frames(), "{}", c.name);
    }
    for a in &plan.audio {
        let bytes = std::fs::read(dir.join(&a.file)).expect("son");
        assert_eq!((bytes.len() - 44) / 4, a.len * 2000, "{}", a.name);
    }
    // Les fichiers de montage disent la même chose que le plan, et leurs chemins relatifs mènent aux médias.
    let xml = crate::xml::parse(&std::fs::read_to_string(dir.join("Mon_film_e.xml")).unwrap()).expect("XML");
    let mut files = Vec::new();
    xml.find_all("pathurl", &mut files);
    assert_eq!(files.len(), 4 + 3, "quatre vidéos, trois sons (chaque fichier est décrit une fois)");
    for p in files {
        assert!(dir.join(&p.text).is_file(), "{} : chemin relatif qui ne mène à rien", p.text);
    }
    let otio: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("Mon_film_e.otio")).unwrap()).unwrap();
    assert_eq!(otio["tracks"]["children"][0]["children"].as_array().unwrap().len(), 5);
    let edl = parse_edl(&std::fs::read_to_string(dir.join("Mon_film_e.edl")).unwrap(), 24);
    assert_eq!(edl.events.last().unwrap().rec_out, 86_400 + plan.total as i64);
    assert!(std::fs::read_to_string(dir.join("LISEZ-MOI.txt")).unwrap().contains("vérifier par Anthony"));
    std::fs::remove_dir_all(&parent).ok();
}

/// `%XX` relus.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).expect("%XX"));
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("UTF-8")
}

#[test]
fn a_real_h264_export_at_30_ntsc_with_absolute_paths() {
    let Some((ffmpeg, enc)) = ffmpeg_and_encoders() else {
        eprintln!("ffmpeg absent : export H.264 sauté");
        return;
    };
    if enc.h264.is_none() {
        return;
    }
    let mut doc = film_with_sound(30);
    dissolve(&mut doc, 2, 10, Align::After);
    let o = Options { sound: true, media: Media::H264, ntsc: true, absolute: true, half: true, edl: false, otio: true, xml: true, start_tc: "00:59:50:00".into(), ..Options::default() };
    let plan = editorial::build_plan(&doc, &o, 0, "Trente").unwrap();
    assert_eq!(plan.rate.ffmpeg(), "30000/1001");
    let parent = temp("h264");
    let mut job = EditorialJob::start(plan.clone(), o, ffmpeg.clone(), enc, &parent).expect("démarré");
    run(&mut job, &doc);
    let report = match job.outcome.clone().expect("terminé") {
        Ok(r) => r,
        Err(e) => panic!("{e}"),
    };
    let names: Vec<String> = report.files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["Trente.xml", "Trente.otio", "LISEZ-MOI.txt"]);
    for c in &plan.clips {
        assert_eq!(c.file.rsplit('.').next(), Some("mp4"));
        assert_eq!(count_frames(&ffmpeg, &report.dir.join(&c.file)), c.file_frames(), "{}", c.name);
    }
    // La cadence des vidéos est bien celle du NTSC : 29,97 i/s (30000/1001).
    let info = std::process::Command::new(&ffmpeg).args(["-hide_banner", "-nostdin", "-i"]).arg(report.dir.join(&plan.clips[0].file)).output().expect("ffmpeg");
    let info = String::from_utf8_lossy(&info.stderr).into_owned();
    assert!(info.contains("29.97 fps"), "{info}");
    // Chemins absolus : ils mènent aux fichiers.
    let xml = crate::xml::parse(&std::fs::read_to_string(report.dir.join("Trente.xml")).unwrap()).expect("XML");
    let mut urls = Vec::new();
    xml.find_all("pathurl", &mut urls);
    assert_eq!(urls.len(), 4 + 3);
    for u in urls {
        let path = percent_decode(u.text.strip_prefix("file://localhost/").unwrap_or_else(|| panic!("{}", u.text)));
        assert!(Path::new(&path).is_file(), "{path} : le chemin absolu ne mène à rien");
    }
    let otio: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(report.dir.join("Trente.otio")).unwrap()).unwrap();
    let url = otio["tracks"]["children"][0]["children"][0]["media_reference"]["target_url"].as_str().unwrap().to_string();
    assert!(url.starts_with("file://localhost/"), "{url}");
    assert_eq!(xml.path("sequence/timecode/string").unwrap().text, "00:59:50:00");
    assert_eq!(xml.path("sequence/rate/ntsc").unwrap().text, "TRUE");
    std::fs::remove_dir_all(&parent).ok();
}

#[test]
fn cancelling_removes_what_the_export_wrote_and_its_folder() {
    let Some((ffmpeg, enc)) = ffmpeg_and_encoders() else {
        eprintln!("ffmpeg absent : annulation sautée");
        return;
    };
    let media = if enc.prores { Media::ProRes } else if enc.h264.is_some() { Media::H264 } else { return };
    let doc = film_with_sound(24);
    let o = Options { sound: true, media, ..Options::default() };
    let plan = editorial::build_plan(&doc, &o, 0, "Annulé").unwrap();
    let parent = temp("annule");
    let mut job = EditorialJob::start(plan, o, ffmpeg, enc, &parent).expect("démarré");
    let dir = job.dir.clone();
    assert!(dir.join("media").is_dir());
    // Un pas : le premier fichier est entamé, ffmpeg tourne, le fil du son écrit.
    job.step(&doc, Duration::from_millis(5));
    if !job.running() {
        eprintln!("l'export a fini avant qu'on l'annule : annulation non essayée");
        std::fs::remove_dir_all(&parent).ok();
        return;
    }
    assert_eq!(job.stage, Stage::Video);
    job.cancel();
    assert_eq!(job.outcome.clone().unwrap().err().as_deref(), Some("Export annulé."));
    assert!(!dir.exists(), "le dossier créé par l'export est retiré : {:?}", std::fs::read_dir(&dir).map(|d| d.flatten().map(|e| e.path()).collect::<Vec<_>>()));
    std::fs::remove_dir_all(&parent).ok();
}

#[test]
fn an_export_that_cannot_start_says_why() {
    let Some((ffmpeg, mut enc)) = ffmpeg_and_encoders() else { return };
    let doc = film(24);
    let plan = plan_of(&doc, &opts());
    enc.prores = false;
    let e = EditorialJob::start(plan, opts(), ffmpeg, enc, &temp("sans-prores")).err().expect("refusé");
    assert!(e.contains("ProRes"), "{e}");
}

// ------------------------------------------------------------------------------------ la fenêtre

#[test]
fn the_window_opens_from_its_action_and_shows_a_plan() {
    let dir = std::env::temp_dir().join(format!("shot-montage-fenetre-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = film(24);
    app.cur_panel = 0;
    assert!(crate::keymap::action_info("fichier.exporter_montage").is_some_and(|a| a.implemented));
    app.run_action("fichier.exporter_montage");
    assert!(app.editorial.is_some());
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 800.0))), time: Some(0.0), ..Default::default() };
    for _ in 0..3 {
        let _ = ctx.run(input.clone(), |ctx| app.editorial_window(ctx));
    }
    assert!(app.editorial.is_some(), "la fenêtre reste ouverte");
}
