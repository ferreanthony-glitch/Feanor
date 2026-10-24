//! Essais de l'export de la planche des vignettes (`planche.rs`, `plancheui.rs`) : options, annotations décochées d'office,
//! marie-louise, résolutions, PDF, tâche de fond (annuler efface tout, le projet reste libre).

use eframe::egui::{pos2, vec2, Pos2};
use image::{Rgb, RgbImage};

use super::*;
use crate::annotations::COULEUR_PAR_DEFAUT;
use crate::board::BoardSettings;
use crate::perf_tests::{se_reposer, Banc};
use crate::vector::VPoint;
use crate::vignettes_tests::{ouvert, quatre_plans};

fn reglages() -> BoardSettings {
    BoardSettings { card_w: 200.0, columns: 3, rows: 0 }
}

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-planche-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

fn tout(doc: &Document) -> Vec<usize> {
    (0..doc.panels.len()).collect()
}

/// Une image unie par vignette, de couleurs différentes.
fn vignettes(p: &Planche, s: f32) -> Vec<RgbImage> {
    (0..p.cartes.len())
        .map(|k| {
            let [w, h] = taille_vignette(p, k, s);
            RgbImage::from_pixel(w, h, Rgb([20 + 30 * k as u8, 90, 200 - 20 * k as u8]))
        })
        .collect()
}

fn pixel(img: &RgbImage, p: &Planche, s: f32, at: Pos2) -> [u8; 3] {
    let (x, y) = (((at.x - p.cadre.left()) * s) as u32, ((at.y - p.cadre.top()) * s) as u32);
    img.get_pixel(x.min(img.width() - 1), y.min(img.height() - 1)).0
}

fn rouge(c: [u8; 3]) -> bool {
    c[0] > 180 && c[1] < 90 && c[2] < 90
}

fn opt() -> Options {
    Options::default()
}

#[test]
fn the_defaults_match_the_board_and_leave_the_annotations_out() {
    let o = Options::default();
    assert!(o.numeros && o.notes && o.barres && o.cadres);
    assert!(!o.annotations, "annotations décochées d'office");
    assert!(!o.marie);
    assert_eq!(o.resolution, Resolution::Dpi(150));
    assert_eq!(o.sortie, Sortie::Png);
}

#[test]
fn one_card_per_image_of_the_scope_laid_out_like_the_board() {
    let doc = quatre_plans();
    let film = construire(&doc, &tout(&doc), &reglages(), &opt(), &doc.annotations);
    assert_eq!(film.cartes.len(), 6, "une vignette par image de storyboard");
    assert_eq!(film.layout, board::layout(&doc, &reglages()), "la disposition de la planche, sans note");
    assert_eq!(film.images(), vec![(0, 0), (0, film.cartes[1].place.frame), (0, film.cartes[2].place.frame), (1, 0), (2, 0), (3, 0)]);
    // Une étendue : seuls ces plans, rangés comme sur une planche qui n'aurait qu'eux.
    let deux = construire(&doc, &[2, 3], &reglages(), &opt(), &doc.annotations);
    assert_eq!(deux.cartes.len(), 2);
    assert_eq!(deux.images(), vec![(2, 0), (3, 0)]);
    assert_eq!(deux.cartes[0].place.row, 0);
    assert!(deux.cadre.height() < film.cadre.height() + 1.0);
}

#[test]
fn the_image_shows_the_cards_the_bars_and_the_numbers_only_when_asked() {
    let doc = quatre_plans();
    let o = opt();
    let p = construire(&doc, &tout(&doc), &reglages(), &o, &doc.annotations);
    let s = 2.0;
    let v = vignettes(&p, s);
    let img = composer(&p, &v, &o, s).expect("image");
    assert_eq!((img.width(), img.height()), ((p.cadre.width() * s).ceil() as u32, (p.cadre.height() * s).ceil() as u32));
    // Chaque vignette est à sa place, dans sa couleur.
    for (k, c) in p.cartes.iter().enumerate() {
        assert_eq!(pixel(&img, &p, s, c.place.thumb.center()), v[k].get_pixel(0, 0).0, "vignette {k}");
    }
    // La page est blanche dans son coin.
    assert_eq!(pixel(&img, &p, s, p.layout.pages[0].left_top() + vec2(2.0, 2.0)), [255, 255, 255]);
    // La barre entre deux images d'un même plan est grise ; sans l'option, rien.
    let barre = |k: usize| p.layout.bars[k].bar.center();
    assert_eq!(pixel(&img, &p, s, barre(0)), [188, 188, 188], "même plan : grise");
    let sans_barres = composer(&p, &v, &Options { barres: false, ..o.clone() }, s).unwrap();
    assert_eq!(pixel(&sans_barres, &p, s, barre(0)), [255, 255, 255]);
    assert_ne!(img, sans_barres);
    // Les numéros : de l'encre au-dessus de la première vignette de chaque plan, rien sans l'option.
    let sans_numeros = composer(&p, &v, &Options { numeros: false, ..o.clone() }, s).unwrap();
    let etiquette = p.cartes[0].place.label();
    let encre = |im: &RgbImage| {
        let (x0, y0) = (((etiquette.left() - p.cadre.left()) * s) as u32, ((etiquette.top() - p.cadre.top()) * s) as u32);
        (0..(etiquette.width() * s) as u32).any(|dx| (0..(etiquette.height() * 0.8 * s) as u32).any(|dy| im.get_pixel(x0 + dx, y0 + dy).0[0] < 200))
    };
    assert!(encre(&img) && !encre(&sans_numeros));
}

#[test]
fn notes_make_taller_rows_and_are_drawn_only_when_asked() {
    let mut doc = quatre_plans();
    doc.panels[1].notes = "Léa entre par la gauche, hésite devant la porte, puis referme doucement derrière elle sans se retourner jamais".into();
    let avec = construire(&doc, &tout(&doc), &reglages(), &opt(), &doc.annotations);
    let sans = construire(&doc, &tout(&doc), &reglages(), &Options { notes: false, ..opt() }, &doc.annotations);
    let carte = avec.cartes.iter().find(|c| c.place.plan == doc.panels[1].id).unwrap();
    let note: Vec<&String> = carte.champs.iter().flat_map(|(_, _, l)| l.iter()).collect();
    assert!(note.len() == 1 && note[0].ends_with('…'), "une seule ligne, coupée par « … » : {note:?}");
    assert!(avec.cadre.height() > sans.cadre.height(), "la rangée descend de ce que la note prend");
    let s = 1.5;
    let o = opt();
    let a = composer(&avec, &vignettes(&avec, s), &o, s).unwrap();
    let r = carte.place.note;
    let (x0, y0) = (((r.left() - avec.cadre.left()) * s) as u32, ((r.top() - avec.cadre.top()) * s) as u32);
    let sombre = (0..(r.width() * s) as u32).any(|dx| (0..(r.height() * s) as u32).any(|dy| a.get_pixel(x0 + dx, y0 + dy).0[0] < 160));
    assert!(sombre, "la note est écrite sous l'image");
    // Sans l'option, aucune zone de texte : ni ligne, ni espace sous les images (la planche est plus basse).
    assert!(sans.cartes.iter().all(|c| !c.place.a_du_texte() && c.champs.is_empty()), "sans l'option, pas de zone de texte");
    let b = composer(&sans, &vignettes(&sans, s), &Options { notes: false, ..o }, s).unwrap();
    assert!(b.height() < a.height(), "sans l'option, la planche se resserre : {} contre {}", b.height(), a.height());
}

#[test]
fn wrapping_notes_keeps_three_lines_and_an_ellipsis() {
    let l = lignes_de("un deux trois quatre cinq six sept huit neuf dix onze douze treize quatorze quinze seize dix-sept", 10.0, 60.0, 3);
    assert_eq!(l.len(), 3);
    assert!(l[2].ends_with('…'));
    assert_eq!(lignes_de("court", 10.0, 200.0, 3), vec!["court".to_string()]);
    assert_eq!(lignes_de("a\nb", 10.0, 200.0, 3), vec!["a".to_string(), "b".to_string()]);
    assert!(lignes_de("   ", 10.0, 200.0, 3).is_empty());
    let t = lignes_de("a b c", 10.0, 1.0, 3);
    assert_eq!(t.len(), 3);
    assert!(!t[2].ends_with('…'), "{t:?}");
}

#[test]
fn annotations_are_left_out_by_default_and_drawn_over_everything_when_asked() {
    let mut doc = quatre_plans();
    let layout = board::layout(&doc, &reglages());
    // Un trait qui part de la vignette 1, traverse l'écart et finit dans la 2.
    let (a, b) = (layout.cards[1].thumb.center(), layout.cards[2].thumb.center());
    let trait_rouge: Vec<VPoint> = (0..20).map(|k| VPoint { x: a.x + (b.x - a.x) * k as f32 / 19.0, y: a.y + 10.0 * (k as f32 / 19.0).sin(), r: 3.0 }).collect();
    assert!(doc.annotations.ajouter(&layout, &trait_rouge, COULEUR_PAR_DEFAUT));
    let s = 1.5;
    let o = opt();
    let sans_trait = quatre_plans();
    let p_sans = construire(&sans_trait, &tout(&sans_trait), &reglages(), &o, &sans_trait.annotations);
    let p_defaut = construire(&doc, &tout(&doc), &reglages(), &o, &doc.annotations);
    let img_sans = composer(&p_sans, &vignettes(&p_sans, s), &o, s).unwrap();
    let img_defaut = composer(&p_defaut, &vignettes(&p_defaut, s), &o, s).unwrap();
    assert_eq!(img_defaut, img_sans, "annotations décochées : l'image est celle d'une planche sans annotation");
    let avec = Options { annotations: true, ..o.clone() };
    let p_avec = construire(&doc, &tout(&doc), &reglages(), &avec, &doc.annotations);
    let img_avec = composer(&p_avec, &vignettes(&p_avec, s), &avec, s).unwrap();
    assert_ne!(img_avec, img_sans);
    let rouges_dans = |im: &RgbImage, p: &Planche, r: Rect| {
        let (x0, y0) = (((r.left() - p.cadre.left()) * s) as u32, ((r.top() - p.cadre.top()) * s) as u32);
        (0..(r.width() * s) as u32).any(|dx| (0..(r.height() * s) as u32).any(|dy| rouge(im.get_pixel(x0 + dx, y0 + dy).0)))
    };
    let ecart = pos2((layout.cards[1].thumb.right() + layout.cards[2].thumb.left()) * 0.5, a.y);
    let zone_ecart = Rect::from_center_size(ecart, vec2(4.0, 30.0));
    assert!(rouges_dans(&img_avec, &p_avec, zone_ecart), "le trait passe par-dessus l'écart");
    assert!(rouges_dans(&img_avec, &p_avec, layout.cards[1].thumb));
    assert!(!rouges_dans(&img_sans, &p_sans, zone_ecart));
}

#[test]
fn marie_louise_paints_everything_outside_the_cases_and_masks_overflowing_strokes() {
    let mut doc = quatre_plans();
    let layout = board::layout(&doc, &reglages());
    let (a, b) = (layout.cards[1].thumb.center(), layout.cards[2].thumb.center());
    let trait_rouge: Vec<VPoint> = (0..20).map(|k| VPoint { x: a.x + (b.x - a.x) * k as f32 / 19.0, y: a.y + 30.0 * (k as f32 / 19.0).sin(), r: 4.0 }).collect();
    doc.annotations.ajouter(&layout, &trait_rouge, COULEUR_PAR_DEFAUT);
    let s = 1.5;
    let o = Options { annotations: true, marie: true, marie_couleur: [0, 0, 0], ..opt() };
    let p = construire(&doc, &tout(&doc), &reglages(), &o, &doc.annotations);
    let img = composer(&p, &vignettes(&p, s), &o, s).unwrap();
    // Dehors des vignettes (et de leur filet) : noir partout.
    let dans = |x: u32, y: u32| {
        p.cartes.iter().any(|c| {
            let r = c.place.thumb;
            let (px, py) = (x as f32 / s + p.cadre.left(), y as f32 / s + p.cadre.top());
            px >= r.left() - 2.0 && px <= r.right() + 2.0 && py >= r.top() - 2.0 && py <= r.bottom() + 2.0
        })
    };
    let mut hors_noir = 0;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if !dans(x, y) && img.get_pixel(x, y).0 != [0, 0, 0] {
                hors_noir += 1;
            }
        }
    }
    assert_eq!(hors_noir, 0, "tout ce qui est hors des vignettes est au noir");
    // Dedans, le trait se voit encore ; les vignettes sont intactes ailleurs.
    let c1 = p.cartes[1].place.thumb;
    assert!((0..60).any(|k| rouge(pixel(&img, &p, s, pos2(c1.center().x + k as f32 - 10.0, c1.center().y + (k as f32 / 6.0).sin() * 3.0)))), "le trait reste dans la vignette");
    assert_eq!(pixel(&img, &p, s, p.cartes[4].place.thumb.center()), [20 + 30 * 4, 90, 200 - 80]);
    // Une autre couleur de passe-partout.
    let creme = Options { marie_couleur: [250, 240, 200], ..o.clone() };
    let img2 = composer(&p, &vignettes(&p, s), &creme, s).unwrap();
    assert_eq!(img2.get_pixel(1, 1).0, [250, 240, 200]);
    // Avec la marie-louise, le cadre ne s'agrandit pas pour les traits qui débordent.
    assert_eq!(p.cadre, p.layout.pages.iter().fold(Rect::NOTHING, |r, q| r.union(*q)));
}

#[test]
fn resolution_by_dpi_by_width_and_the_bounds() {
    let cadre = Rect::from_min_size(pos2(10.0, 20.0), vec2(960.0, 600.0));
    let (s, dims, reduit) = dimensions(cadre, Resolution::Dpi(150));
    assert!((s - 150.0 / 96.0).abs() < 1e-6 && dims == [1500, 938] && reduit.is_none(), "{s} {dims:?}");
    let (_, dims, _) = dimensions(cadre, Resolution::Dpi(300));
    assert_eq!(dims, [3000, 1875]);
    let (_, dims, _) = dimensions(cadre, Resolution::Largeur(2000));
    assert_eq!(dims[0], 2000);
    let (s, dims, reduit) = dimensions(cadre, Resolution::Largeur(40_000));
    assert!(dims[0] as f32 * dims[1] as f32 <= MAX_PIXELS * 1.001 && dims[0] as f32 <= MAX_COTE + 1.0, "{dims:?}");
    assert!(s < 40_000.0 / 960.0);
    assert!(reduit.is_some_and(|r| r.contains("réduite")));
}

#[test]
fn pdf_pieces_cut_between_the_pages_of_the_board() {
    let pages = [Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 300.0)), Rect::from_min_max(pos2(0.0, 336.0), pos2(100.0, 636.0)), Rect::from_min_max(pos2(0.0, 672.0), pos2(100.0, 972.0))];
    let c = coupures(0.0, 972.0, &pages, 700.0);
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].0, 0.0);
    assert!((c[0].1 - 654.0).abs() < 1e-3, "coupé dans l'écart entre les pages 2 et 3 : {c:?}");
    assert_eq!(c[1].1, 972.0);
    assert_eq!(coupures(0.0, 972.0, &pages, 2000.0), vec![(0.0, 972.0)]);
    let c = coupures(0.0, 300.0, &pages[..1], 120.0);
    assert_eq!(c.len(), 3);
    let total: f32 = c.iter().map(|(a, b)| b - a).sum();
    assert!((total - 300.0).abs() < 1e-3);
}

#[test]
fn every_output_is_a_valid_file() {
    let doc = quatre_plans();
    let o = opt();
    let p = construire(&doc, &tout(&doc), &reglages(), &o, &doc.annotations);
    let s = 1.0;
    let img = composer(&p, &vignettes(&p, s), &o, s).unwrap();
    let png = encoder(&p, &img, Sortie::Png, "T", s).unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
    assert_eq!(image::load_from_memory(&png).unwrap().to_rgb8(), img, "le PNG est exact");
    let jpg = encoder(&p, &img, Sortie::Jpeg, "T", s).unwrap();
    assert_eq!(&jpg[..2], &[0xFF, 0xD8]);
    assert_eq!(image::load_from_memory(&jpg).unwrap().to_rgb8().dimensions(), img.dimensions());
    let page = String::from_utf8_lossy(&encoder(&p, &img, Sortie::PdfUnePage, "T", s).unwrap()).to_string();
    assert!(page.starts_with("%PDF") && page.contains("/Count 1"));
    assert!(page.contains(&format!("/MediaBox [0 0 {:.2} {:.2}]", p.cadre.width() * 0.75, p.cadre.height() * 0.75)), "page à la taille de la planche");
    let a4 = String::from_utf8_lossy(&encoder(&p, &img, Sortie::PdfA4, "T", s).unwrap()).to_string();
    let a3 = String::from_utf8_lossy(&encoder(&p, &img, Sortie::PdfA3, "T", s).unwrap()).to_string();
    assert!(a4.contains("/MediaBox [0 0 842.00 595.00]") && a3.contains("/MediaBox [0 0 1191.00 842.00]"));
    let pages = |t: &str| t.matches("/Type /Page ").count();
    assert!(pages(&a4) >= 1 && pages(&a3) >= 1 && pages(&a3) <= pages(&a4));
}

#[test]
fn the_camera_frames_show_where_the_camera_moves_in_blue_with_the_footprint_of_the_board() {
    use crate::camera::{Camera, CameraKey};
    let mut doc = quatre_plans();
    let [fw, _] = doc.frame;
    let (w, h) = (doc.width, doc.height);
    let n = doc.panels[2].frame_count();
    let depart = Camera::centered(w, h);
    let arrivee = Camera { x: depart.x + fw as f32 * 0.5, ..depart };
    doc.panels[2].camera = vec![CameraKey::new(0, depart), CameraKey::new(n - 1, arrivee)];
    let o = opt();
    let p = construire(&doc, &tout(&doc), &reglages(), &o, &doc.annotations);
    let mobile = p.cartes.iter().find(|c| c.plan_reel == 2).unwrap();
    let mv = mobile.mouvement.as_ref().expect("la caméra bouge dans ce plan");
    assert_eq!(mv.cadres.len(), 2);
    assert!(p.cartes.iter().filter(|c| c.plan_reel != 2).all(|c| c.mouvement.is_none()), "les autres plans n'ont pas d'empreinte");
    // Le rangement est celui de l'écran (sans les cases pâles) : l'empreinte est dans la planche, jamais sur une autre vignette.
    assert!(p.cadre.contains_rect(mobile.place.foot), "l'empreinte tient dans l'image exportée");
    for (i, a) in p.cartes.iter().enumerate() {
        for b in p.cartes.iter().skip(i + 1) {
            assert!(!a.place.foot.shrink(0.5).intersects(b.place.foot.shrink(0.5)), "deux empreintes se chevauchent");
        }
    }
    let sans = construire(&doc, &tout(&doc), &reglages(), &Options { cadres: false, ..o.clone() }, &doc.annotations);
    assert!(sans.cartes.iter().all(|c| c.mouvement.is_none()));
    let s = 2.0;
    let a = composer(&p, &vignettes(&p, s), &o, s).unwrap();
    let b = composer(&sans, &vignettes(&sans, s), &Options { cadres: false, ..o.clone() }, s).unwrap();
    let r = mobile.place.foot;
    let (x0, y0) = (((r.left() - p.cadre.left()) * s) as u32, ((r.top() - p.cadre.top()) * s) as u32);
    let (lw, lh) = ((r.width() * s) as u32, (r.height() * s) as u32);
    let trouve = |f: &dyn Fn([u8; 3]) -> bool| (0..lw).any(|dx| (0..lh).any(|dy| f(a.get_pixel(x0 + dx, y0 + dy).0)));
    assert!(trouve(&|c| c[2] > 200 && c[0] < 100 && c[1] > 100 && c[1] < 180), "les cadres sont tracés en bleu fin (#378ADD)");
    assert!(!trouve(&|c| c[0] > 200 && c[1] > 90 && c[1] < 150 && c[2] < 60), "plus de cadre orange");
    assert_ne!(a, b);
}

// ------------------------------------------------------------------------------------ la tâche de fond

fn lancer(b: &mut Banc, job: crate::export::ExportJob) {
    b.app.start_job(job);
    let debut = std::time::Instant::now();
    while b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) && debut.elapsed() < std::time::Duration::from_secs(60) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    b.chauffer(2);
}

fn vignette_centre(b: &Banc, k: usize) -> Pos2 {
    b.app.board_layout().cards[k].thumb.center()
}

#[test]
fn the_export_job_writes_the_file_in_the_background_and_leaves_the_project_free() {
    let dir = dossier("tache");
    let mut b = ouvert(quatre_plans(), 3);
    let plans = tout(&b.app.doc);
    let sortie = dir.join("planche.png");
    let job = b.app.planche_job(&plans, &opt(), &sortie).expect("tâche");
    assert_eq!(job.unit, "Vignette");
    lancer(&mut b, job);
    let fin = b.app.export_job.as_ref().and_then(|j| j.outcome.clone()).expect("fini");
    assert!(fin.as_ref().is_ok_and(|m| m.contains("Planche exportée")), "{fin:?}");
    let img = image::open(&sortie).expect("PNG lisible").to_rgb8();
    let p = construire(&b.app.doc, &plans, &b.app.layout.board_settings(), &opt(), &b.app.doc.annotations);
    assert_eq!(img.width(), dimensions(p.cadre, Resolution::Dpi(150)).1[0]);
    assert!(!dir.join("planche.png.part").exists(), "aucun fichier .part");
    assert!(!b.app.modal_open(), "le projet est libre une fois copié");
    // Annotations présentes mais décochées : le fichier est octet pour octet celui d'une planche sans annotation.
    let (a, c) = (vignette_centre(&b, 1), vignette_centre(&b, 2));
    let lay = b.app.board_layout();
    let trait_rouge: Vec<VPoint> = (0..10).map(|k| VPoint { x: a.x + (c.x - a.x) * k as f32 / 9.0, y: a.y, r: 3.0 }).collect();
    b.app.doc.annotations.ajouter(&lay, &trait_rouge, COULEUR_PAR_DEFAUT);
    let avec = dir.join("avec.png");
    b.app.export_job = None;
    let job = b.app.planche_job(&plans, &opt(), &avec).unwrap();
    lancer(&mut b, job);
    assert!(std::fs::read(&sortie).unwrap() == std::fs::read(&avec).unwrap(), "les annotations ne sont pas dans l'export par défaut");
    // Cochées : le fichier change.
    let coche = dir.join("coche.png");
    b.app.export_job = None;
    let job = b.app.planche_job(&plans, &Options { annotations: true, ..opt() }, &coche).unwrap();
    lancer(&mut b, job);
    assert!(std::fs::read(&sortie).unwrap() != std::fs::read(&coche).unwrap());
    b.app.export_job = None;
    assert!(se_reposer(&mut b, 80) < 80, "l'écran se repose");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelling_the_export_leaves_nothing_behind_and_keeps_the_old_file() {
    let dir = dossier("annule");
    let mut b = ouvert(quatre_plans(), 3);
    let plans = tout(&b.app.doc);
    let sortie = dir.join("planche.png");
    std::fs::write(&sortie, b"ancienne planche").unwrap();
    let job = b.app.planche_job(&plans, &opt(), &sortie).unwrap();
    b.app.start_job(job);
    if let Some(j) = b.app.export_job.as_mut() {
        j.cancel();
    }
    let fin = b.app.export_job.as_ref().and_then(|j| j.outcome.clone()).expect("fini");
    assert!(fin.is_err());
    std::thread::sleep(std::time::Duration::from_millis(300));
    b.chauffer(3);
    assert!(!dir.join("planche.png.part").exists(), "pas de .part");
    assert!(std::fs::read(&sortie).unwrap() == b"ancienne planche", "l'ancienne planche est intacte");
    assert!(b.app.planche_job(&[], &opt(), &sortie).is_err(), "sans plan : une erreur claire");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_pdf_export_job_writes_a_pdf() {
    let dir = dossier("pdf");
    let mut b = ouvert(quatre_plans(), 3);
    let plans = tout(&b.app.doc);
    let sortie = dir.join("planche.pdf");
    let o = Options { sortie: Sortie::PdfA4, ..opt() };
    let job = b.app.planche_job(&plans, &o, &sortie).unwrap();
    lancer(&mut b, job);
    let octets = std::fs::read(&sortie).expect("PDF");
    assert!(octets.starts_with(b"%PDF") && octets.ends_with(b"%%EOF\n"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn both_windows_open_from_their_actions_draw_and_the_hub_lists_them() {
    let mut b = ouvert(quatre_plans(), 3);
    b.garder_textes = true;
    b.app.run_action("fichier.exporter");
    b.chauffer(4);
    assert!(b.textes.iter().any(|t| t.contains("Planche des vignettes")) && b.textes.iter().any(|t| t.contains("Color script")), "le menu Exporter les propose");
    b.app.export_hub = false;
    b.app.run_action("fichier.exporter_planche");
    b.chauffer(4);
    assert!(b.app.vignettes.planche.ouverte);
    assert!(b.textes.iter().any(|t| t.contains("Tout le film (4)")) && b.textes.iter().any(|t| t.contains("Avec les annotations")), "{:?}", b.textes);
    b.app.run_action("fichier.exporter_colorscript");
    b.chauffer(4);
    assert!(b.app.vignettes.colorscript.ouverte);
    assert!(b.textes.iter().any(|t| t.contains("Titres des séquences")));
    assert!(b.textes.iter().any(|t| t.contains("images,") || t.contains("image,")), "le résumé dit combien d'images : {:?}", b.textes);
    // Les réglages d'origine : annotations décochées, bande éteinte, fond noir.
    assert!(!b.app.vignettes.planche.opt.annotations && !b.app.vignettes.colorscript.opt.bande);
    assert_eq!(b.app.vignettes.colorscript.opt.fond, [0, 0, 0]);
    // Les plans choisis ne sont proposés qu'à partir de deux.
    b.app.board_sel = vec![b.app.doc.panels[0].id, b.app.doc.panels[1].id];
    b.app.run_action("fichier.exporter_planche");
    assert_eq!(b.app.vignettes.planche.etendue, crate::relectureui::Etendue::Choisis);
    b.app.vignettes.planche.ouverte = false;
    b.app.vignettes.colorscript.ouverte = false;
    b.chauffer(3);
    assert!(se_reposer(&mut b, 80) < 80);
}