//! Tests des transitions du montage : le temps (où tombe la zone, ce que montre chaque image), le
//! mélange d'une image (fondu enchaîné à 50 % = la moyenne, fondu au noir, volet), le fichier
//! `transitions.json` et sa protection, l'export, l'annulation.

use std::time::Duration;

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};
use crate::export::{self, ExportJob, Range};
use crate::project;
use crate::transitions::{self, blend_images, mix_in, zones, Align, Kind, Mix, Transition, DEFAULT_FRAMES, FILE, MAX_FRAMES};

/// Un film de plans de `len` images, chacun rempli d'une couleur unie sur toute la feuille.
fn solid_film(colors: &[[u8; 3]], len: usize) -> Document {
    let mut doc = Document::with_frame(32, 24, 12);
    for _ in 1..colors.len() {
        let n = doc.panels.len() - 1;
        doc.add_scene_after(n);
    }
    for (p, c) in colors.iter().enumerate() {
        let id = doc.new_cel(LayerKind::Bitmap);
        let r = &mut doc.cels.get_mut(&id).unwrap().raster;
        for y in 0..doc.height {
            for x in 0..doc.width {
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % 64) * 64 + x % 64] = Color32::from_rgba_premultiplied(c[0], c[1], c[2], 255);
            }
        }
        let mut l = Layer::new("Couleur", LayerKind::Bitmap);
        l.frames = vec![Some(id); len];
        doc.panels[p].layers = vec![l];
        doc.panels[p].length = len;
    }
    doc
}

/// Trois plans de 10 images, sans dessin.
fn film() -> Document {
    let mut doc = Document::new(32, 32, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for p in doc.panels.iter_mut() {
        p.set_length(10);
    }
    doc
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-transitions-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn flat(c: [u8; 4], n: usize) -> Vec<Color32> {
    vec![Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]); n]
}

// -------------------------------------------------------------------------------- le temps

#[test]
fn a_centred_transition_overlaps_the_cut_and_never_moves_a_plan() {
    let mut doc = film();
    let before = doc.total_frames();
    let id = doc.set_transition(1, Kind::Dissolve).unwrap();
    assert_eq!(doc.transition(id).unwrap().frames, DEFAULT_FRAMES);
    let z = zones(&doc);
    assert_eq!(z.len(), 1);
    // La coupe est à l'image 10 : six images avant, six après.
    assert_eq!((z[0].cut, z[0].start, z[0].end), (10, 4, 16));
    // Le film ne change pas de durée : ni le son ni les commentaires ne bougent.
    assert_eq!(doc.total_frames(), before);
    assert_eq!((doc.panel_start(1), doc.panel_start(2)), (10, 20));
}

#[test]
fn the_alignment_puts_the_zone_before_or_after_the_cut() {
    let mut doc = film();
    let id = doc.set_transition(1, Kind::Dissolve).unwrap();
    for (align, want) in [(Align::Before, (2, 10)), (Align::After, (10, 18)), (Align::Centre, (6, 14))] {
        let t = doc.transitions.iter_mut().find(|t| t.id == id).unwrap();
        t.frames = 8;
        t.align = align;
        let z = zones(&doc)[0];
        assert_eq!((z.start, z.end), want, "{align:?}");
    }
}

#[test]
fn a_long_transition_is_cut_to_its_two_plans() {
    let mut doc = film();
    doc.panels[0].set_length(3);
    doc.panels[1].set_length(4);
    let id = doc.set_transition(1, Kind::Dissolve).unwrap();
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().frames = 40;
    // Elle ne sort ni du plan sortant (3 images) ni du plan entrant (4).
    let z = zones(&doc)[0];
    assert_eq!((z.start, z.end), (0, 7));
}

#[test]
fn fades_at_the_ends_of_the_film_use_the_first_and_last_frames() {
    let mut doc = film();
    let start = doc.set_transition(0, Kind::Black).unwrap();
    let end = doc.set_transition(3, Kind::White).unwrap();
    assert!(doc.transitions.iter().any(|t| t.id == start && t.out.is_none()));
    assert!(doc.transitions.iter().any(|t| t.id == end && t.inn.is_none()));
    // (Douze images, mais le premier plan n'en a que dix : le fondu ne sort pas du plan qu'il habille.)
    let z = zones(&doc);
    assert_eq!((z[0].start, z[0].end), (0, 10));
    assert_eq!((z[1].start, z[1].end), (20, 30));
    // Un fondu enchaîné et un volet ont besoin de deux plans.
    assert!(doc.set_transition(0, Kind::Dissolve).is_none());
    assert!(doc.set_transition(3, Kind::Wipe).is_none());
    assert!(Kind::CutBlack.allowed(false) && !Kind::Wipe.allowed(false));
}

#[test]
fn each_frame_of_the_zone_shows_both_plans_and_holds_the_missing_images() {
    let mut doc = film();
    doc.set_transition(1, Kind::Dissolve);
    let zs = zones(&doc);
    // Avant la coupe (image 8) : le plan sortant est vivant (image 8), l'entrant tient sa première.
    let m = mix_in(&zs, 8).unwrap().1;
    assert_eq!((m.a, m.b), (Some((0, 8)), Some((1, 0))));
    // Après la coupe (image 12) : le sortant tient sa dernière image, l'entrant est vivant.
    let m = mix_in(&zs, 12).unwrap().1;
    assert_eq!((m.a, m.b), (Some((0, 9)), Some((1, 2))));
    // Hors de la zone : rien.
    assert!(mix_in(&zs, 3).is_none() && mix_in(&zs, 16).is_none());
    assert!(doc.mix_at(12).is_some() && doc.mix_at(25).is_none());
}

#[test]
fn the_weight_of_the_incoming_plan_grows_and_is_symmetric_around_the_cut() {
    let mut doc = film();
    doc.set_transition(1, Kind::Dissolve);
    let z = zones(&doc)[0];
    let ms: Vec<f32> = (z.start..z.end).map(|g| z.mix(g).unwrap().m).collect();
    assert_eq!(ms.len(), 12);
    assert!(ms.windows(2).all(|w| w[0] < w[1]), "{ms:?}");
    assert!(ms[0] > 0.0 && ms[11] < 1.0, "ni A seul ni B seul : {ms:?}");
    assert!((ms[0] + ms[11] - 1.0).abs() < 1e-6 && (ms[5] + ms[6] - 1.0).abs() < 1e-6, "{ms:?}");
}

#[test]
fn plans_that_are_no_longer_neighbours_have_no_zone() {
    let mut doc = film();
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    doc.set_transition(1, Kind::Dissolve);
    assert_eq!(zones(&doc).len(), 1);
    // Un plan est déplacé entre les deux : la transition est gardée mais ne joue plus.
    doc.move_plans(&[b], None);
    assert_eq!(doc.panels[0].id, a);
    assert!(zones(&doc).is_empty());
    assert_eq!(doc.transitions.len(), 1, "gardée : l'annulation du déplacement la rend");
    // Un plan supprimé emporte ses transitions.
    doc.remove_panels(&[a]);
    assert!(doc.transitions.is_empty());
}

#[test]
fn the_nearest_cut_is_found_from_the_playhead() {
    let doc = film();
    assert_eq!(doc.nearest_cut(0), 0);
    assert_eq!(doc.nearest_cut(4), 0);
    assert_eq!(doc.nearest_cut(6), 1);
    assert_eq!(doc.nearest_cut(10), 1);
    assert_eq!(doc.nearest_cut(24), 2);
    assert_eq!(doc.nearest_cut(29), 3);
    assert_eq!(doc.nearest_cut(500), 3);
}

#[test]
fn dragging_an_edge_changes_the_duration_keeping_the_centre() {
    // Centrée : la durée est le double de la distance du bord à la coupe.
    assert_eq!(Align::Centre.frames_for_edge(10, 4, true), 12);
    assert_eq!(Align::Centre.frames_for_edge(10, 16, false), 12);
    assert_eq!(Align::Centre.frames_for_edge(10, 10, true), 1, "au moins une image");
    assert_eq!(Align::Before.frames_for_edge(10, 4, true), 6);
    assert_eq!(Align::After.frames_for_edge(10, 20, false), 10);
    assert_eq!(Align::Centre.frames_for_edge(150, 0, true), MAX_FRAMES);
}

// ----------------------------------------------------------------------------------- mélange

#[test]
fn a_dissolve_at_fifty_percent_is_the_average() {
    let a = flat([200, 40, 0, 255], 12);
    let b = flat([100, 80, 250, 255], 12);
    let mix = Mix { a: Some((0, 0)), b: Some((1, 0)), wipe: false, m: 0.5, color: [0, 0, 0], k: 0.0 };
    let out = blend_images(&mix, Some(&a), Some(&b), [4, 3]);
    for px in &out {
        let [r, g, bl, al] = px.to_array();
        assert!((r as i32 - 150).abs() <= 1 && (g as i32 - 60).abs() <= 1 && (bl as i32 - 125).abs() <= 1 && al == 255, "{px:?}");
    }
    // Et le poids 0 donne A, le poids 1 donne B.
    assert_eq!(blend_images(&Mix { m: 0.0, ..mix }, Some(&a), Some(&b), [4, 3]), a);
    assert_eq!(blend_images(&Mix { m: 1.0, ..mix }, Some(&a), Some(&b), [4, 3]), b);
}

#[test]
fn the_middle_frame_of_an_odd_dissolve_is_exactly_half_way() {
    let mut doc = film();
    let id = doc.set_transition(1, Kind::Dissolve).unwrap();
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().frames = 11;
    let z = zones(&doc)[0];
    assert_eq!(z.len(), 11);
    let m = z.mix(z.start + 5).unwrap();
    assert_eq!(m.m, 0.5);
}

#[test]
fn a_fade_to_black_ends_on_a_black_frame_and_from_black_starts_on_one() {
    let mut doc = film();
    doc.set_transition(0, Kind::Black);
    doc.set_transition(3, Kind::Black);
    let zs = zones(&doc);
    let (from_black, to_black) = (zs[0], zs[1]);
    // Début : la première image est toute noire, puis le plan entrant s'allume.
    let first = from_black.mix(0).unwrap();
    assert_eq!((first.a, first.b, first.k), (None, Some((0, 0)), 1.0));
    let ks: Vec<f32> = (from_black.start..from_black.end).map(|g| from_black.mix(g).unwrap().k).collect();
    assert!(ks.windows(2).all(|w| w[0] > w[1]) && ks[9] > 0.0, "{ks:?}");
    // Fin : le plan sortant s'éteint jusqu'à la dernière image, toute noire.
    let last = to_black.mix(29).unwrap();
    assert_eq!((last.a, last.b, last.k), (Some((2, 9)), None, 1.0));
    let img = flat([10, 200, 90, 255], 6);
    let black = blend_images(&last, Some(&img), None, [3, 2]);
    assert!(black.iter().all(|p| *p == Color32::BLACK), "{black:?}");
    // À mi-chemin, la moitié du noir : la couleur est divisée par deux.
    let half = blend_images(&Mix { k: 0.5, ..last }, Some(&img), None, [3, 2]);
    let [r, g, b, a] = half[0].to_array();
    assert!((r as i32 - 5).abs() <= 1 && (g as i32 - 100).abs() <= 1 && (b as i32 - 45).abs() <= 1 && a == 255, "{:?}", half[0]);
    // Le blanc de même.
    let white = blend_images(&Mix { k: 1.0, color: [255, 255, 255], ..last }, Some(&img), None, [3, 2]);
    assert!(white.iter().all(|p| *p == Color32::WHITE));
}

#[test]
fn a_dip_to_black_between_plans_goes_through_black_in_the_middle() {
    let mut doc = film();
    doc.set_transition(1, Kind::Black);
    let z = zones(&doc)[0];
    let first = z.mix(z.start).unwrap();
    assert!(first.a.is_some() && first.b.is_none() && first.k > 0.0 && first.k < 0.5);
    // Les deux images du milieu sont presque toutes noires (12 images : 12/13 du noir).
    let (m5, m6) = (z.mix(z.start + 5).unwrap(), z.mix(z.start + 6).unwrap());
    assert!(m5.k > 0.9 && m6.k > 0.9, "{} {}", m5.k, m6.k);
    // L'entrant s'allume dans la seconde moitié.
    assert!(m5.a.is_some() && m6.b.is_some() && m6.a.is_none());
    let last = z.mix(z.end - 1).unwrap();
    assert!(last.b.is_some() && last.k < 0.2);
}

#[test]
fn the_black_transition_blacks_out_every_frame_of_the_zone() {
    let mut doc = film();
    doc.set_transition(1, Kind::CutBlack);
    let z = zones(&doc)[0];
    for g in z.start..z.end {
        let m = z.mix(g).unwrap();
        assert_eq!((m.k, m.color), (1.0, [0, 0, 0]), "image {g}");
    }
}

#[test]
fn a_wipe_shows_the_incoming_plan_on_the_left() {
    let a = flat([255, 0, 0, 255], 8);
    let b = flat([0, 0, 255, 255], 8);
    let mix = Mix { a: Some((0, 0)), b: Some((1, 0)), wipe: true, m: 0.5, color: [0, 0, 0], k: 0.0 };
    let out = blend_images(&mix, Some(&a), Some(&b), [4, 2]);
    for row in 0..2 {
        assert_eq!(out[row * 4], b[0]);
        assert_eq!(out[row * 4 + 1], b[0]);
        assert_eq!(out[row * 4 + 2], a[0]);
        assert_eq!(out[row * 4 + 3], a[0]);
    }
}

// ------------------------------------------------------------------------------- le fichier

#[test]
fn the_sidecar_round_trips_with_the_project() {
    let mut doc = film();
    let id = doc.set_transition(1, Kind::Dissolve).unwrap();
    let t = doc.transitions.iter_mut().find(|t| t.id == id).unwrap();
    t.frames = 9;
    t.align = Align::After;
    doc.set_transition(0, Kind::White);
    doc.set_transition(3, Kind::CutBlack);
    let dir = temp("aller-retour");
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
    assert!(text.contains("\"transitions\"") && text.contains("enchaine") && text.contains("apres"), "{text}");
    let loaded = project::load(&manifest).unwrap();
    assert!(!loaded.warnings.iter().any(|w| w.contains(FILE)), "{:?}", loaded.warnings);
    let sorted = |d: &Document| {
        let mut v: Vec<Transition> = d.transitions.clone();
        v.sort_by_key(|t| t.id);
        v
    };
    assert_eq!(sorted(&loaded.doc), sorted(&doc));
    assert_eq!(zones(&loaded.doc).len(), 3);
    // Les numéros suivants ne retombent pas sur ceux des transitions relues.
    let mut back = loaded.doc;
    let n = back.set_transition(2, Kind::Wipe).unwrap();
    assert_eq!(back.transitions.iter().filter(|t| t.id == n).count(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn without_a_transition_there_is_no_file_and_the_manifest_version_is_unchanged() {
    let mut doc = film();
    let dir = temp("sans-fichier");
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists());
    assert_eq!(project::VERSION, 31, "pas de version du projet pour les transitions (la 27 est celle des calques d'effets, la 31 celle des effets du trait) : un fichier à côté suffit");
    // Des transitions, puis plus aucune : le fichier part.
    let id = doc.set_transition(1, Kind::Dissolve).unwrap();
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(dir.join(FILE).exists());
    doc.remove_transition(id);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_damaged_or_newer_sidecar_opens_without_transitions_and_is_never_deleted() {
    let mut doc = film();
    let dir = temp("protege");
    let manifest = dir.join("F.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();

    // Abîmé : le projet s'ouvre, on le dit.
    std::fs::write(dir.join(FILE), "{ pas du json").unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.doc.transitions.is_empty());
    assert!(loaded.warnings.iter().any(|w| w.contains(FILE)), "{:?}", loaded.warnings);
    // Un enregistrement sans transition ne l'efface pas.
    project::save(&loaded.doc, &manifest, &mut Default::default()).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), "{ pas du json");

    // D'une version plus récente : même chose, avec le bon message.
    let newer = r#"{"format":"shot-transitions","version":99,"transitions":[]}"#;
    std::fs::write(dir.join(FILE), newer).unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.warnings.iter().any(|w| w.contains("plus récente")), "{:?}", loaded.warnings);
    project::save(&loaded.doc, &manifest, &mut Default::default()).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), newer);

    // Une transition neuve à écrire : l'ancien fichier est mis de côté, pas écrasé.
    doc.set_transition(1, Kind::Dissolve);
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("transitions.json.illisible")).unwrap(), newer);
    assert!(project::load(&manifest).unwrap().doc.transitions.len() == 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn reading_the_file_repairs_what_can_be_repaired() {
    let text = r#"{"format":"shot-transitions","version":1,"transitions":[
        {"id":5,"sortant":1,"entrant":2,"sorte":"enchaine","images":9999},
        {"id":5,"sortant":2,"entrant":3,"sorte":"volet","images":4},
        {"id":0,"sortant":null,"entrant":1,"sorte":"enchaine","images":0},
        {"id":7,"sortant":1,"entrant":2,"sorte":"noir","images":3},
        {"id":8,"sortant":null,"entrant":null,"sorte":"noir","images":3}
    ]}"#;
    let list = transitions::parse(text).unwrap();
    // Une seule transition par coupe (la seconde de 1→2 est ignorée) et aucune sans plan.
    assert_eq!(list.len(), 3, "{list:?}");
    assert_eq!(list[0].frames, MAX_FRAMES, "durée bornée");
    let ids: std::collections::HashSet<u64> = list.iter().map(|t| t.id).collect();
    assert_eq!(ids.len(), 3, "numéros uniques : {list:?}");
    assert!(!ids.contains(&0));
    // Un fondu enchaîné au bord du film devient un fondu au noir (jamais de zone sans deux plans).
    let edge = list.iter().find(|t| t.out.is_none()).unwrap();
    assert_eq!((edge.kind, edge.frames), (Kind::Black, 1));
    assert!(transitions::parse("{}").is_err() && transitions::parse(r#"{"format":"autre","version":1}"#).is_err());
}

// -------------------------------------------------------------------------------- l'export

#[test]
fn the_export_renders_the_transitions_frame_by_frame() {
    let red = [255, 0, 0];
    let blue = [0, 0, 255];
    let mut doc = solid_film(&[red, blue], 8);
    let id = doc.set_transition(1, Kind::Dissolve).unwrap();
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().frames = 4;
    // Zone [6, 10) : poids de B 1/5, 2/5, 3/5, 4/5.
    let frames = export::frames_for(&doc, Range::Montage, 0);
    assert_eq!(frames.len(), 16);
    let dir = temp("export");
    let mut job = ExportJob::images(&dir, "film", frames.clone(), doc.frame, true).with_transitions(&doc);
    run(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let px = |i: usize| image::open(dir.join(format!("film_{i:04}.png"))).unwrap().to_rgba8().get_pixel(10, 10).0;
    // Hors de la zone : les plans tels quels.
    assert_eq!(px(1), [255, 0, 0, 255]);
    assert_eq!(px(6), [255, 0, 0, 255], "image 5 (0-based), juste avant la zone");
    assert_eq!(px(16), [0, 0, 255, 255]);
    // Dans la zone : le mélange attendu, au bit près de l'arrondi.
    for (k, g) in (6..10).enumerate() {
        let w = (k + 1) as f32 / 5.0;
        let want_b = (255.0 * w).round() as i32;
        let got = px(g + 1);
        assert!((got[2] as i32 - want_b).abs() <= 1 && (got[0] as i32 - (255 - want_b)).abs() <= 1 && got[3] == 255, "image {g} : {got:?} (B à {w})");
    }
    // Sans les transitions : un film coupé net.
    let dir2 = temp("export-sans");
    let mut plain = ExportJob::images(&dir2, "film", frames, doc.frame, true);
    run(&mut plain, &doc);
    let plain_px = |i: usize| image::open(dir2.join(format!("film_{i:04}.png"))).unwrap().to_rgba8().get_pixel(10, 10).0;
    assert_eq!((plain_px(8), plain_px(9)), ([255, 0, 0, 255], [0, 0, 255, 255]));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&dir2).ok();
}

#[test]
fn the_export_fades_the_film_to_black_at_its_end_and_wipes() {
    let mut doc = solid_film(&[[200, 100, 50], [50, 100, 200]], 6);
    let id = doc.set_transition(2, Kind::Black).unwrap();
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().frames = 3;
    let w = doc.set_transition(1, Kind::Wipe).unwrap();
    {
        let t = doc.transitions.iter_mut().find(|t| t.id == w).unwrap();
        t.frames = 3;
        t.align = Align::After;
    }
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let dir = temp("export-noir");
    let mut job = ExportJob::images(&dir, "f", frames, doc.frame, true).with_transitions(&doc);
    run(&mut job, &doc);
    let img = |i: usize| image::open(dir.join(format!("f_{i:04}.png"))).unwrap().to_rgba8();
    // La dernière image est toute noire, l'avant-dernière aux deux tiers du noir, la première du
    // fondu à un tiers (le plan entrant est (50, 100, 200)).
    assert_eq!(img(12).get_pixel(5, 5).0, [0, 0, 0, 255]);
    let two_thirds = img(11).get_pixel(5, 5).0;
    assert!((two_thirds[0] as i32 - 17).abs() <= 2 && (two_thirds[2] as i32 - 67).abs() <= 2, "{two_thirds:?}");
    let third = img(10).get_pixel(5, 5).0;
    assert!((third[0] as i32 - 33).abs() <= 2 && (third[2] as i32 - 133).abs() <= 2, "{third:?}");
    // Volet (image 6, 7, 8 : B revient de la gauche) : à l'image 7, la moitié gauche est B.
    let wiped = img(8);
    assert_eq!(wiped.get_pixel(1, 5).0, [50, 100, 200, 255], "à gauche : le plan entrant");
    assert_eq!(wiped.get_pixel(30, 5).0, [200, 100, 50, 255], "à droite : le plan sortant tenu");
    std::fs::remove_dir_all(&dir).ok();
}

/// Fait tourner un export jusqu'au bout, comme l'interface.
fn run(job: &mut ExportJob, doc: &Document) {
    let t = std::time::Instant::now();
    while job.outcome.is_none() {
        job.step(doc, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(60), "export trop long");
    }
}

// --------------------------------------------------------------------------- l'annulation

fn app_with(doc: Document, name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-transitions-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_frame = 0;
    app
}

#[test]
fn every_edit_of_a_transition_is_one_named_undo_step() {
    let mut app = app_with(film(), "annuler");
    // Tête de lecture sur la coupe du plan 2 (image 10) : la commande vise cette coupe.
    app.cur_panel = 1;
    app.cur_frame = 0;
    app.run_action("transition.ajouter");
    assert_eq!(app.doc.transitions.len(), 1);
    assert_eq!(app.doc.transitions[0].kind, Kind::Dissolve, "un fondu enchaîné entre deux plans");
    let (done, _) = app.history.names();
    assert_eq!(done.last().copied(), Some("Ajouter une transition : fondu enchaîné"));

    app.run_action("transition.volet");
    assert_eq!(app.doc.transitions[0].kind, Kind::Wipe);
    assert_eq!(app.history.names().0.last().copied(), Some("Transition : volet"));

    let id = app.doc.transitions[0].id;
    app.set_transition_align(id, Align::Before);
    assert_eq!(app.doc.transitions[0].align, Align::Before);
    assert_eq!(app.history.names().0.last().copied(), Some("Calage de la transition"));

    app.delete_transition(id);
    assert!(app.doc.transitions.is_empty());
    assert_eq!(app.history.names().0.last().copied(), Some("Supprimer la transition"));

    // Chaque étape se défait une à une, dans l'ordre.
    app.undo();
    assert_eq!(app.doc.transitions[0].align, Align::Before);
    app.undo();
    assert_eq!((app.doc.transitions[0].kind, app.doc.transitions[0].align), (Kind::Wipe, Align::Centre));
    app.undo();
    assert_eq!(app.doc.transitions[0].kind, Kind::Dissolve);
    app.undo();
    assert!(app.doc.transitions.is_empty());
    app.redo();
    assert_eq!(app.doc.transitions.len(), 1);
}

#[test]
fn the_toolbar_command_picks_the_cut_nearest_to_the_playhead_and_deletes_with_the_key() {
    let mut app = app_with(film(), "proche");
    // Image 13 du film : la coupe la plus proche est celle d'entre les plans 2 et 3 (image 10).
    app.cur_panel = 1;
    app.cur_frame = 3;
    app.run_action("transition.ajouter");
    let t = app.doc.transitions[0].clone();
    assert_eq!((t.out, t.inn), (Some(app.doc.panels[0].id), Some(app.doc.panels[1].id)));
    // Une seconde demande ne double pas la transition : elle la choisit.
    app.run_action("transition.ajouter");
    assert_eq!(app.doc.transitions.len(), 1);
    assert_eq!(app.trans.sel, Some(t.id));
    // Suppr, le montage ayant la main et la transition choisie : elle part.
    app.panel_focus = true;
    app.trans.focus = true;
    app.run_action("edition.supprimer");
    assert!(app.doc.transitions.is_empty());
    // Au début du film, la commande pose un fondu au noir.
    app.cur_panel = 0;
    app.cur_frame = 0;
    app.run_action("transition.ajouter");
    assert_eq!((app.doc.transitions[0].kind, app.doc.transitions[0].out), (Kind::Black, None));
}

#[test]
fn transitions_are_part_of_the_undoable_structure() {
    let mut doc = film();
    let before = doc.structure();
    doc.set_transition(1, Kind::Dissolve);
    let after = doc.structure();
    assert!(before.transitions.is_empty() && after.transitions.len() == 1);
    let back = doc.replace_structure(before);
    assert!(doc.transitions.is_empty() && back.transitions.len() == 1);
    doc.replace_structure(back);
    assert_eq!(doc.transitions.len(), 1);
}

// ------------------------------------------------------------------- la carte graphique

/// Ce que la carte graphique affiche d'une image de transition (la feuille entière, 1 pixel pour 1).
fn gpu_mix(rs: &eframe::egui_wgpu::RenderState, gpu: &mut crate::gpu::GpuCanvas, doc: &Document, mix: &Mix, wipe_u: [f32; 2]) -> Vec<Color32> {
    use crate::gpu::{DisplayMix, MixLayer};
    use eframe::egui::{pos2, vec2, Rect};
    let plan_of = |at: (usize, usize)| crate::render::panel_plan_at(doc, at.0, at.1, None);
    let first = plan_of(mix.a.unwrap());
    let second = mix.b.map(plan_of);
    let mut both = first.clone();
    if let Some(s) = &second {
        both.items.extend(s.items.iter().cloned());
    }
    let resolved = gpu.sync(doc, &both, true, &[], &[], 0);
    let n = first.items.len();
    let (w, h) = (doc.width as u32, doc.height as u32);
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(w as f32, h as f32));
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    let display = DisplayMix { wipe: mix.wipe, m: mix.m, grey: mix.color[0] as f32 / 255.0, k: mix.k, wipe_u };
    let a = MixLayer { plan: &first, resolved: &resolved[..n], corners, ws: None };
    let b = second.as_ref().map(|p| MixLayer { plan: p, resolved: &resolved[n..], corners, ws: None });
    let callback = gpu.paint_mix(a, b, Some(&display), rect, [doc.width, doc.height]);
    crate::gpu_tests::read_callback(rs, w, h, rect, callback)
}

#[test]
fn the_graphics_card_mixes_the_plans_like_the_processor() {
    let Some(rs) = crate::gpu_tests::headless() else {
        eprintln!("aucune carte graphique : essai sauté");
        return;
    };
    let doc = solid_film(&[[200, 40, 0], [100, 80, 250]], 4);
    let mut gpu = crate::gpu::GpuCanvas::new(&rs);
    let at = |img: &[Color32], x: usize, y: usize| img[y * doc.width + x].to_array();
    let close = |got: [u8; 4], want: [i32; 3], tol: i32| (0..3).all(|k| (got[k] as i32 - want[k]).abs() <= tol) && got[3] == 255;
    let base = Mix { a: Some((0, 0)), b: Some((1, 0)), wipe: false, m: 0.5, color: [0, 0, 0], k: 0.0 };
    let (red, blue) = ([200, 40, 0], [100, 80, 250]);

    // Fondu enchaîné à 50 % : la moyenne.
    let img = gpu_mix(&rs, &mut gpu, &doc, &base, [0.25, 0.75]);
    assert!(close(at(&img, 32, 24), [150, 60, 125], 3), "{:?}", at(&img, 32, 24));
    // Poids 0 et 1 : un plan, puis l'autre (le plan tenu d'un côté ne se recompose pas).
    let img = gpu_mix(&rs, &mut gpu, &doc, &Mix { m: 0.0, ..base }, [0.25, 0.75]);
    assert!(close(at(&img, 32, 24), red, 2));
    let img = gpu_mix(&rs, &mut gpu, &doc, &Mix { m: 1.0, ..base }, [0.25, 0.75]);
    assert!(close(at(&img, 32, 24), blue, 2));
    // Fondu au noir (un seul plan sous la couche) : la moitié du noir, puis tout.
    let single = Mix { b: None, k: 0.5, ..base };
    let img = gpu_mix(&rs, &mut gpu, &doc, &single, [0.25, 0.75]);
    assert!(close(at(&img, 32, 24), [100, 20, 0], 3), "{:?}", at(&img, 32, 24));
    let img = gpu_mix(&rs, &mut gpu, &doc, &Mix { k: 1.0, ..single }, [0.25, 0.75]);
    assert!(close(at(&img, 32, 24), [0, 0, 0], 1));
    // Fondu au blanc.
    let img = gpu_mix(&rs, &mut gpu, &doc, &Mix { k: 1.0, color: [255, 255, 255], ..single }, [0.25, 0.75]);
    assert!(close(at(&img, 32, 24), [255, 255, 255], 1));
    // Volet : le cadre filmé va de la colonne 16 à la colonne 48 de la feuille ; à mi-chemin, B à gauche
    // de la colonne 32, A à droite.
    let img = gpu_mix(&rs, &mut gpu, &doc, &Mix { wipe: true, ..base }, [0.25, 0.75]);
    assert!(close(at(&img, 20, 24), blue, 2), "{:?}", at(&img, 20, 24));
    assert!(close(at(&img, 44, 24), red, 2), "{:?}", at(&img, 44, 24));
}

#[test]
fn the_new_commands_are_in_the_palette_and_the_shortcut_file() {
    for id in [
        "transition.ajouter",
        "transition.supprimer",
        "transition.fondu_enchaine",
        "transition.fondu_noir",
        "transition.fondu_blanc",
        "transition.volet",
        "transition.noir",
        "transition.apercu",
    ] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
}

#[test]
fn the_badges_follow_the_zones_and_can_be_picked() {
    let mut doc = film();
    doc.set_transition(1, Kind::Dissolve);
    let zs = zones(&doc);
    let bs = crate::transitionsui::badges(&zs, 100.0, 4.0, 50.0);
    assert_eq!(bs.len(), 1);
    // Zone [4, 16) à 4 points par image, à partir de x = 100.
    assert_eq!((bs[0].body.left(), bs[0].body.right()), (116.0, 164.0));
    use crate::transitionsui::{badge_hit, BadgeHit};
    assert_eq!(badge_hit(&bs, egui::pos2(140.0, 50.0)).map(|h| h.1), Some(BadgeHit::Body));
    assert_eq!(badge_hit(&bs, egui::pos2(117.0, 50.0)).map(|h| h.1), Some(BadgeHit::GripLeft));
    assert_eq!(badge_hit(&bs, egui::pos2(163.0, 50.0)).map(|h| h.1), Some(BadgeHit::GripRight));
    assert!(badge_hit(&bs, egui::pos2(140.0, 80.0)).is_none() && badge_hit(&bs, egui::pos2(90.0, 50.0)).is_none());
    // Une transition courte en zoom arrière reste assez large pour être prise.
    let t = doc.transitions[0].id;
    doc.transitions.iter_mut().find(|x| x.id == t).unwrap().frames = 2;
    let small = crate::transitionsui::badges(&zones(&doc), 0.0, 0.5, 10.0);
    assert!(small[0].body.width() >= 18.0);
    // Les coupes se trouvent à ±4 points de la jonction des blocs.
    let blocks = [egui::Rect::from_min_max(egui::pos2(10.0, 0.0), egui::pos2(60.0, 40.0)), egui::Rect::from_min_max(egui::pos2(61.0, 0.0), egui::pos2(100.0, 40.0))];
    assert_eq!(crate::transitionsui::cut_at(&blocks, egui::pos2(60.0, 20.0), 0.0, 40.0), Some(1));
    assert_eq!(crate::transitionsui::cut_at(&blocks, egui::pos2(9.0, 20.0), 0.0, 40.0), Some(0));
    assert_eq!(crate::transitionsui::cut_at(&blocks, egui::pos2(101.0, 20.0), 0.0, 40.0), Some(2));
    assert_eq!(crate::transitionsui::cut_at(&blocks, egui::pos2(40.0, 20.0), 0.0, 40.0), None);
}
