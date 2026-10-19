//! Incrustations (`incrustation.rs`) : le texte du timecode, du numéro d'image, du nom du plan… ; la
//! boîte et sa place dans le cadre ; ce que l'export grave et ce qu'il laisse selon « Seulement à
//! l'écran / à l'export ».

use std::time::Duration;

use eframe::egui::Color32;

use crate::calqueffet::CalqueEffet;
use crate::document::{Document, Layer, LayerKind};
use crate::export::{self, ExportJob, Range};
use crate::export_tests::doc_with_drawing;
use crate::incrustation::{self, Ancre, Incrustation, Portee, Quand, Source};

#[test]
fn the_timecode_counts_hours_minutes_seconds_and_frames_at_the_project_rate() {
    assert_eq!(incrustation::timecode(0, 24), "00:00:00:00");
    assert_eq!(incrustation::timecode(23, 24), "00:00:00:23");
    assert_eq!(incrustation::timecode(24, 24), "00:00:01:00");
    assert_eq!(incrustation::timecode(25, 24), "00:00:01:01");
    assert_eq!(incrustation::timecode(24 * 60 + 5, 24), "00:01:00:05");
    assert_eq!(incrustation::timecode(24 * 3600 * 2 + 24 * 61 + 7, 24), "02:01:01:07");
    // D'autres cadences entières (pas de timecode « drop frame » : SHOT n'a pas de 29,97).
    assert_eq!(incrustation::timecode(26, 25), "00:00:01:01");
    assert_eq!(incrustation::timecode(30 * 59 + 12, 30), "00:00:59:12");
    assert_eq!(incrustation::timecode(10, 1), "00:00:10:00");
    // Une cadence nulle ne divise pas par zéro.
    assert_eq!(incrustation::timecode(5, 0), "00:00:05:00");
}

#[test]
fn the_date_is_the_calendar_date() {
    assert_eq!(incrustation::date_text(0), "1970-01-01");
    assert_eq!(incrustation::date_text(951_782_400), "2000-02-29");
    assert_eq!(incrustation::date_text(1_760_000_000), "2025-10-09");
    assert_eq!(incrustation::date_text(4_102_444_799), "2099-12-31");
}

fn two_plans() -> Document {
    let mut doc = doc_with_drawing(2, 10);
    doc.properties.title = "Mon film".into();
    doc
}

#[test]
fn the_text_follows_the_source_and_the_scope() {
    let doc = two_plans();
    let mut inc = Incrustation::default();
    let text = |inc: &Incrustation, panel: usize, frame: usize| incrustation::text_for(inc, &doc, panel, frame, 1_760_000_000);
    // Le film : l'image 3 du second plan est la 13e (12 à partir de 0) ; le plan : la 3e.
    assert_eq!(text(&inc, 1, 3), "00:00:01:01", "à 12 i/s : 13 images = 1 s et 1 image");
    inc.portee = Portee::Plan;
    assert_eq!(text(&inc, 1, 3), "00:00:00:03");
    inc.source = Source::NumeroImage;
    assert_eq!(text(&inc, 1, 3), "0004", "l'image de départ du projet est 1");
    inc.portee = Portee::Montage;
    assert_eq!(text(&inc, 1, 3), "0014");
    inc.source = Source::NomPlan;
    assert_eq!(text(&inc, 0, 0), "Plan 1");
    assert_eq!(text(&inc, 1, 0), "Plan 1", "deux plans de la même scène portent le même nom");
    inc.source = Source::NomSequence;
    assert_eq!(text(&inc, 0, 0), "Séquence 1");
    inc.source = Source::Titre;
    assert_eq!(text(&inc, 0, 0), "Mon film");
    inc.source = Source::Date;
    assert_eq!(text(&inc, 0, 0), "2025-10-09");
    inc.source = Source::Libre;
    inc.texte = "{titre} — {plan} — {timecode} — {image} — {date} — {sequence} — {inconnu}".into();
    assert_eq!(text(&inc, 1, 3), "Mon film — Plan 1 — 00:00:01:01 — 0014 — 2025-10-09 — Séquence 1 — {inconnu}");
}

#[test]
fn the_box_takes_its_place_from_one_of_nine_anchors() {
    let frame = [300, 200];
    let mut inc = Incrustation { taille: 10.0, marge: 5.0, fond: Some([0, 0, 0, 255]), ..Default::default() };
    let mut seen = Vec::new();
    for a in Ancre::ALL {
        inc.ancre = a;
        let b = incrustation::render_box(&inc, "12:34", frame).expect("un texte");
        let (w, h) = (b.raster.width as i32, b.raster.height as i32);
        assert!(b.x >= 0 && b.y >= 0 && b.x + w <= 300 && b.y + h <= 200, "{a:?} reste dans le cadre");
        seen.push((b.x, b.y));
        // La boîte est pleine (le fond) : son coin est opaque.
        assert_eq!(b.raster.get(0, 0).a(), 255);
    }
    // Neuf places différentes, rangées de haut en bas et de gauche à droite.
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 9);
    assert!(seen[0].0 < seen[1].0 && seen[1].0 < seen[2].0 && seen[0].1 < seen[3].1 && seen[3].1 < seen[6].1);
    // La marge : 5 % de la hauteur du cadre (10 pixels) du coin haut-gauche ; au centre, centrée.
    assert_eq!(seen[0], (10, 10));
    let b = incrustation::render_box(&Incrustation { ancre: Ancre::Centre, ..inc.clone() }, "12:34", frame).unwrap();
    assert!((b.x * 2 + b.raster.width as i32 - 300).abs() <= 1 && (b.y * 2 + b.raster.height as i32 - 200).abs() <= 1);
    // Rien à dire, rien à poser.
    assert!(incrustation::render_box(&inc, "", frame).is_none());
    assert!(incrustation::render_box(&inc, "  ", frame).is_none());
}

#[test]
fn the_text_is_drawn_in_its_colour_with_or_without_a_box() {
    let frame = [300, 200];
    let plain = Incrustation { taille: 20.0, couleur: [255, 0, 0, 255], fond: None, ..Default::default() };
    let b = incrustation::render_box(&plain, "88", frame).unwrap();
    let mut red = 0;
    let mut other = 0;
    for y in 0..b.raster.height {
        for x in 0..b.raster.width {
            let p = b.raster.get(x, y);
            if p.a() == 255 {
                if p == Color32::from_rgb(255, 0, 0) {
                    red += 1;
                } else {
                    other += 1;
                }
            }
        }
    }
    assert!(red > 50, "des pixels rouges pleins : {red}");
    assert_eq!(other, 0, "sans boîte, rien d'autre que le texte");
    assert_eq!(b.raster.get(0, 0).a(), 0, "le coin est transparent sans boîte");
    // Le même texte, deux fois : les mêmes pixels (l'écran et l'export posent la même chose).
    let again = incrustation::render_box(&plain, "88", frame).unwrap();
    assert!((0..b.raster.height).all(|y| (0..b.raster.width).all(|x| b.raster.get(x, y) == again.raster.get(x, y))));
}

#[test]
fn burning_writes_the_box_into_the_final_picture_only_where_it_lies() {
    let frame = [64, 48];
    let inc = Incrustation { ancre: Ancre::BasDroite, taille: 25.0, marge: 5.0, couleur: [255, 255, 255, 255], fond: Some([0, 0, 0, 255]), ..Default::default() };
    let b = incrustation::render_box(&inc, "00:00:01:01", frame).unwrap();
    let (bx, by, bw, bh) = (b.x, b.y, b.raster.width as i32, b.raster.height as i32);
    let mut px = vec![Color32::from_rgb(200, 100, 50); frame[0] * frame[1]];
    incrustation::burn(&mut px, frame, &[b]);
    let changed = px.iter().filter(|p| **p != Color32::from_rgb(200, 100, 50)).count();
    assert!(changed > 0);
    for (i, p) in px.iter().enumerate() {
        let (x, y) = ((i % frame[0]) as i32, (i / frame[0]) as i32);
        let inside = x >= bx && x < bx + bw && y >= by && y < by + bh;
        if !inside {
            assert_eq!(*p, Color32::from_rgb(200, 100, 50), "({x}, {y}) hors de la boîte");
        }
    }
}

/// Un projet de deux plans de dix images avec un calque d'incrustation au-dessus dans chacun.
fn with_overlay(quand: Quand) -> Document {
    let mut doc = doc_with_drawing(2, 10);
    for p in 0..2 {
        let cel = doc.new_cel(LayerKind::Bitmap);
        let mut l = Layer::new_effect("Timecode", CalqueEffet::incrustation(
            Incrustation { ancre: Ancre::HautGauche, taille: 12.0, marge: 3.0, couleur: [255, 255, 0, 255], fond: Some([0, 0, 60, 255]), quand, ..Default::default() },
            "Timecode",
        ));
        l.frames = vec![Some(cel); 10];
        doc.panels[p].layers.push(l);
    }
    doc
}

fn run(job: &mut ExportJob, doc: &Document) {
    let t = std::time::Instant::now();
    while job.outcome.is_none() {
        job.step(doc, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(60), "export trop long");
    }
}

fn export_images(doc: &Document, name: &str) -> Vec<image::RgbaImage> {
    let dir = std::env::temp_dir().join(format!("shot-incrust-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let frames = export::frames_for(doc, Range::Montage, 0);
    let mut job = ExportJob::images(&dir, "essai", frames, doc.frame, true);
    run(&mut job, doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let n = doc.total_frames();
    let images = (1..=n).map(|i| image::open(dir.join(format!("essai_{i:04}.png"))).unwrap().to_rgba8()).collect();
    std::fs::remove_dir_all(&dir).ok();
    images
}

#[test]
fn the_export_burns_the_overlay_unless_it_is_screen_only() {
    let both = with_overlay(Quand::Toujours);
    let export_only = with_overlay(Quand::ExportSeulement);
    let screen_only = with_overlay(Quand::EcranSeulement);
    let plain = doc_with_drawing(2, 10);
    let reference = export_images(&plain, "sans");
    let burned = export_images(&both, "toujours");
    let only_export = export_images(&export_only, "export");
    let only_screen = export_images(&screen_only, "ecran");
    assert_eq!((reference.len(), burned.len()), (20, 20));
    // Le timecode est gravé dans chaque image, et change d'une image à l'autre (le texte change).
    for i in 0..20 {
        assert_ne!(burned[i].as_raw(), reference[i].as_raw(), "image {i} : le timecode est gravé");
        assert_eq!(burned[i].as_raw(), only_export[i].as_raw(), "« seulement à l'export » grave aussi");
        assert_eq!(only_screen[i].as_raw(), reference[i].as_raw(), "« seulement à l'écran » ne grave pas");
    }
    assert_ne!(burned[0].as_raw(), burned[1].as_raw(), "00:00:00:00 puis 00:00:00:01");
    // Le coin haut-gauche porte la boîte bleu nuit ; le coin bas-droit n'est pas touché.
    assert_eq!(burned[0].get_pixel(0, 0).0, reference[0].get_pixel(0, 0).0, "hors de la boîte (marge)");
    let boxed = (0..30u32).flat_map(|y| (0..60u32).map(move |x| (x, y))).filter(|&(x, y)| burned[0].get_pixel(x, y).0 == [0, 0, 60, 255]).count();
    assert!(boxed > 100, "la boîte est gravée : {boxed} pixels");
    let (w, h) = (burned[0].width() - 1, burned[0].height() - 1);
    assert_eq!(burned[0].get_pixel(w, h).0, reference[0].get_pixel(w, h).0);
}

#[test]
fn a_hidden_overlay_or_one_without_exposure_is_not_burned() {
    let mut doc = with_overlay(Quand::Toujours);
    assert!(incrustation::any_for(&doc, 0, 0, true));
    assert!(incrustation::any_for(&doc, 0, 0, false));
    // Caché : ni gravé ni montré.
    doc.panels[0].layers.last_mut().unwrap().visible = false;
    assert!(!incrustation::any_for(&doc, 0, 0, true));
    assert!(incrustation::boxes_for(&doc, 0, 0, true, 0).is_empty());
    // Une exposition qui s'arrête à l'image 4 : plus rien ensuite.
    let l = doc.panels[1].layers.last_mut().unwrap();
    for f in 4..10 {
        l.set(f, None);
    }
    assert!(incrustation::any_for(&doc, 1, 3, true));
    assert!(!incrustation::any_for(&doc, 1, 4, true));
    // Les neuf places, les trois « quand » : « Seulement à l'écran » n'est pas à l'export, et réciproquement.
    let doc = with_overlay(Quand::EcranSeulement);
    assert!(incrustation::any_for(&doc, 0, 0, false) && !incrustation::any_for(&doc, 0, 0, true));
    let doc = with_overlay(Quand::ExportSeulement);
    assert!(!incrustation::any_for(&doc, 0, 0, false) && incrustation::any_for(&doc, 0, 0, true));
}

#[test]
fn the_opacity_of_the_layer_fades_the_overlay() {
    let mut doc = with_overlay(Quand::Toujours);
    let full = incrustation::boxes_for(&doc, 0, 0, true, 0);
    doc.panels[0].layers.last_mut().unwrap().opacity = 0.5;
    let half = incrustation::boxes_for(&doc, 0, 0, true, 0);
    assert_eq!(full.len(), 1);
    assert_eq!(half.len(), 1);
    assert_eq!(full[0].raster.get(0, 0).a(), 255);
    assert!((half[0].raster.get(0, 0).a() as i32 - 128).abs() <= 1);
}
