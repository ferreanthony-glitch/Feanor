//! L'affichage d'une vue réduite (`gpu.rs`, `fs_display` ; partie 166.9) : en dessous de 100 %, chaque pixel d'écran couvre plusieurs
//! pixels de la feuille, et tous doivent compter. Sinon un trait fin se casse en pointillés et son épaisseur change d'une ligne à l'autre.
//!
//! Les essais peignent vraiment sur la carte graphique et relisent l'écran (`Banc::capturer`) ; sans carte, ils sont sautés.

use eframe::egui::{pos2, vec2, Pos2, Rect};

use crate::document::{Cel, Document, Layer, LayerKind};
use crate::perf_tests::Banc;
use crate::raster::Raster;

const LARGEUR: usize = 1600;
const HAUTEUR: usize = 1200;
/// Les traits de la feuille, noirs, d'un pixel de large : colonne de départ, et pente (un pixel de côté tous les `pente` pixels de haut).
const TRAITS: [(usize, usize); 5] = [(200, 4), (500, 6), (750, 9), (950, 13), (1150, 17)];
/// Les lignes de la feuille que les traits parcourent.
const HAUT: usize = 200;
const BAS: usize = 1000;

/// La colonne du trait `(x0, pente)` à la ligne `y` de la feuille.
fn colonne(x0: usize, pente: usize, y: usize) -> usize {
    x0 + (y - HAUT) / pente
}

/// Un projet d'un plan, un calque bitmap : les traits fins de `TRAITS`, sur le papier blanc.
fn feuille_de_traits_fins() -> Document {
    let mut doc = Document::new(LARGEUR, HAUTEUR, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 4;
    let mut rgba = vec![0u8; LARGEUR * HAUTEUR * 4];
    for (x0, pente) in TRAITS {
        for y in HAUT..BAS {
            let i = (y * LARGEUR + colonne(x0, pente, y)) * 4;
            rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }
    let mut cel = Cel::new(LARGEUR, HAUTEUR, LayerKind::Bitmap);
    cel.raster = Raster::from_rgba(LARGEUR, HAUTEUR, &rgba);
    let dessin = doc.add_cel(cel);
    let mut calque = Layer::new("Traits", LayerKind::Bitmap);
    calque.set(0, Some(dessin));
    doc.panels[0].layers.push(calque);
    doc
}

/// Le banc sur ce projet, la vue du plan seule (ni zones de sécurité, ni vue nette). Rien sans carte graphique.
fn banc_des_traits() -> Option<Banc> {
    let mut b = crate::perf_tests::banc(feuille_de_traits_fins(), true);
    if b.capturer().is_none() {
        println!("     pas de carte graphique pour les essais : essai sauté");
        return None;
    }
    b.app.set_mode(crate::app::Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.layout.safe_frames = false;
    b.chauffer(10);
    Some(b)
}

/// Pose la vue : ce zoom, cette rotation, le milieu de la feuille au milieu de la zone du dessin.
fn poser_la_vue(b: &mut Banc, zoom: f32, rotation: f32) {
    b.app.zoom_by(zoom / b.app.view.zoom);
    b.app.view.rotation = rotation;
    let (rect, size) = (b.app.canvas_rect, b.app.canvas_size());
    let milieu = b.app.view.to_screen(rect, size, pos2(size.x / 2.0, size.y / 2.0));
    b.app.view.pan += rect.center() - milieu;
    b.chauffer(4);
}

fn ecran(b: &Banc, p: Pos2) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), p)
}

/// La zone du dessin où l'on mesure : loin des bords et du titre du plan (en haut).
fn zone_mesuree(b: &Banc) -> Rect {
    let r = b.app.canvas_rect;
    Rect::from_min_max(r.min + vec2(12.0, 70.0), r.max - vec2(12.0, 12.0))
}

/// L'encre d'un pixel de la capture (1920 × 1080, RGBA) : la part du pixel couverte de noir, en lumière linéaire (0 : papier, 1 : encre).
fn encre(px: &[u8], x: usize, y: usize) -> f32 {
    1.0 - eframe::egui::ecolor::linear_f32_from_gamma_u8(px[(y * 1920 + x) * 4])
}

/// Le moins, le plus et la moyenne de ces mesures, et leur nombre.
fn resume(mesures: &[f32]) -> (f32, f32, f32, usize) {
    let (moins, plus) = mesures.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    (moins, plus, mesures.iter().sum::<f32>() / mesures.len().max(1) as f32, mesures.len())
}

/// La vue droite : pour chaque trait et chaque ligne d'écran qu'il traverse, l'encre de la ligne autour du trait. Un trait d'un pixel de
/// la feuille y laisse, à chaque ligne, une encre égale au zoom (un pixel de large, réduit).
fn encre_par_ligne(b: &Banc, px: &[u8]) -> Vec<f32> {
    let (rect, size, zone) = (b.app.canvas_rect, b.app.canvas_size(), zone_mesuree(b));
    let mut mesures = Vec::new();
    for (x0, pente) in TRAITS {
        for yy in zone.min.y.ceil() as usize..zone.max.y.floor() as usize {
            // (La ligne de la feuille au milieu de cette ligne d'écran ; la vue est droite.)
            let y = b.app.view.to_canvas(rect, size, pos2(rect.center().x, yy as f32 + 0.5)).y;
            if y < HAUT as f32 + 12.0 || y > BAS as f32 - 12.0 {
                continue;
            }
            let x = ecran(b, pos2(x0 as f32 + (y - HAUT as f32) / pente as f32, y)).x;
            if x - 8.0 < zone.min.x || x + 8.0 > zone.max.x {
                continue;
            }
            let (a, z) = ((x - 7.0).round() as usize, (x + 7.0).round() as usize);
            mesures.push((a..=z).map(|xx| encre(px, xx, yy)).sum());
        }
    }
    mesures
}

/// Sur la carte graphique, la vue réduite : un trait fin garde la même encre à chaque ligne d'écran, à tous les zooms. (Avant la
/// partie 166.9, à 37 %, quatre points de la feuille étaient lus par pixel d'écran là où il y en a plus de sept : des lignes entières du
/// trait manquaient, d'autres étaient deux fois trop foncées.)
#[test]
fn une_vue_reduite_garde_les_traits_fins_entiers() {
    let Some(mut b) = banc_des_traits() else { return };
    for zoom in [0.8_f32, 0.5, 0.367, 0.2, 0.11] {
        poser_la_vue(&mut b, zoom, 0.0);
        let px = b.capturer().expect("carte graphique");
        let (moins, plus, moyenne, n) = resume(&encre_par_ligne(&b, &px));
        println!("     zoom {zoom} : {n} lignes mesurées, encre par ligne de {:.2} à {:.2} fois le zoom, {:.3} en moyenne", moins / zoom, plus / zoom, moyenne / zoom);
        assert!(n >= 150, "les traits sont à l'écran au zoom {zoom} : {n} lignes mesurées");
        assert!((moyenne / zoom - 1.0).abs() < 0.05, "au zoom {zoom}, l'encre moyenne d'un trait fin est {:.3} fois ce qu'elle doit être", moyenne / zoom);
        assert!(moins >= 0.8 * zoom && plus <= 1.2 * zoom, "au zoom {zoom}, un trait fin doit garder son encre à chaque ligne : de {:.2} à {:.2} fois le zoom", moins / zoom, plus / zoom);
    }
}

/// À 100 %, vue droite : les pixels de la feuille tels quels, comme avant. Chaque ligne d'écran montre du trait un seul pixel noir, entre
/// des pixels blancs.
#[test]
fn a_cent_pour_cent_les_pixels_de_la_feuille_restent_tels_quels() {
    let Some(mut b) = banc_des_traits() else { return };
    poser_la_vue(&mut b, 1.0, 0.0);
    let px = b.capturer().expect("carte graphique");
    let (rect, size, zone) = (b.app.canvas_rect, b.app.canvas_size(), zone_mesuree(&b));
    let mut lignes = 0;
    for (x0, pente) in TRAITS {
        for yy in zone.min.y.ceil() as usize..zone.max.y.floor() as usize {
            let y = b.app.view.to_canvas(rect, size, pos2(rect.center().x, yy as f32 + 0.5)).y;
            if y < HAUT as f32 + 12.0 || y > BAS as f32 - 12.0 {
                continue;
            }
            let x = ecran(&b, pos2(colonne(x0, pente, y.floor() as usize) as f32 + 0.5, y)).x;
            if x - 8.0 < zone.min.x || x + 8.0 > zone.max.x {
                continue;
            }
            let rouges: Vec<u8> = ((x - 6.0).floor() as usize..=(x + 6.0).floor() as usize).map(|xx| px[(yy * 1920 + xx) * 4]).collect();
            let (noirs, blancs) = (rouges.iter().filter(|v| **v == 0).count(), rouges.iter().filter(|v| **v == 255).count());
            assert!(noirs == 1 && blancs == rouges.len() - 1, "à 100 %, ligne d'écran {yy} : un pixel noir entre des blancs, pas {rouges:?}");
            lignes += 1;
        }
    }
    assert!(lignes >= 150, "les traits sont à l'écran : {lignes} lignes vérifiées");
}

/// L'encre autour de points pris le long de chaque trait (fenêtres de 5 pixels d'écran de côté) : le long d'un trait droit, elle ne
/// doit pas changer, quelle que soit la rotation de la vue.
fn encre_le_long(b: &Banc, px: &[u8]) -> Vec<f32> {
    let zone = zone_mesuree(b);
    let mut mesures = Vec::new();
    for (x0, pente) in TRAITS {
        let mut par_trait = Vec::new();
        for y in (HAUT + 20..BAS - 20).step_by(3) {
            let p = ecran(b, pos2(x0 as f32 + (y - HAUT) as f32 / pente as f32 + 0.5, y as f32 + 0.5));
            if !zone.shrink(6.0).contains(p) {
                continue;
            }
            let (cx, cy) = (p.x.floor() as usize, p.y.floor() as usize);
            par_trait.push((cy - 2..=cy + 2).flat_map(|yy| (cx - 2..=cx + 2).map(move |xx| (xx, yy))).map(|(xx, yy)| encre(px, xx, yy)).sum::<f32>());
        }
        // (Chaque mesure rapportée à la moyenne de son trait : la longueur de trait dans la fenêtre dépend de son inclinaison.)
        let moyenne = par_trait.iter().sum::<f32>() / par_trait.len().max(1) as f32;
        mesures.extend(par_trait.iter().map(|v| v / moyenne.max(1e-6)));
    }
    mesures
}

/// La vue tournée : réduite, un trait fin reste régulier sur toute sa longueur ; à 100 %, il est lissé (des pixels intermédiaires) au
/// lieu de marches.
#[test]
fn une_vue_tournee_reste_reguliere() {
    let Some(mut b) = banc_des_traits() else { return };
    for (zoom, rotation) in [(0.367_f32, 0.3_f32), (0.6, -1.1), (0.25, 0.8)] {
        poser_la_vue(&mut b, zoom, rotation);
        let px = b.capturer().expect("carte graphique");
        let (moins, plus, _, n) = resume(&encre_le_long(&b, &px));
        println!("     zoom {zoom}, vue tournée de {rotation} : {n} fenêtres, encre de {moins:.2} à {plus:.2} fois la moyenne du trait");
        assert!(n >= 150, "les traits sont à l'écran : {n} fenêtres");
        assert!(moins >= 0.7 && plus <= 1.3, "au zoom {zoom}, vue tournée de {rotation}, un trait fin doit rester régulier : de {moins:.2} à {plus:.2} fois sa moyenne");
    }
    // À 100 %, tournée : plus de marches (des pixels tout noirs ou tout blancs seulement), le trait est lissé.
    poser_la_vue(&mut b, 1.0, 0.3);
    let px = b.capturer().expect("carte graphique");
    let zone = zone_mesuree(&b);
    let (mut gris, mut noirs) = (0, 0);
    for yy in zone.min.y.ceil() as usize..zone.max.y.floor() as usize {
        for xx in zone.min.x.ceil() as usize..zone.max.x.floor() as usize {
            match px[(yy * 1920 + xx) * 4] {
                0..=20 => noirs += 1,
                40..=215 => gris += 1,
                _ => {}
            }
        }
    }
    println!("     100 %, vue tournée : {gris} pixels intermédiaires, {noirs} noirs");
    assert!(gris > 4 * noirs.max(50), "à 100 %, la vue tournée est lissée : {gris} pixels intermédiaires pour {noirs} noirs");
}

/// Quand l'image se pose en pixels nets, quand elle se pose lissée (`gpu::pose_nette`) : nette à 100 % et au-delà, vue droite ; lissée
/// dès que la vue la réduit, tournée jusqu'à 102 %, ou en perspective.
#[test]
fn l_image_se_pose_nette_ou_lissee_selon_la_vue() {
    let taille = (1600u32, 1200u32);
    // Les coins de l'image à l'écran, en points, pour ce zoom (points par pixel de l'image), cette rotation et ce retournement.
    let coins = |zoom: f32, rotation: f32, retournee: bool| -> [[f32; 4]; 4] {
        let (s, c) = rotation.sin_cos();
        [(0.0, 0.0), (1600.0, 0.0), (1600.0, 1200.0), (0.0, 1200.0)].map(|(x, y): (f32, f32)| {
            let x = if retournee { 1600.0 - x } else { x };
            [300.5 + zoom * (x * c - y * s), 80.25 + zoom * (x * s + y * c), 0.0, 0.0]
        })
    };
    let nette = |zoom: f32, rotation: f32, retournee: bool, perspective: bool, ppp: f32| crate::gpu::pose_nette(&coins(zoom, rotation, retournee), perspective, ppp, taille);
    let quart = std::f32::consts::FRAC_PI_2;
    // Vue droite : nette à 100 % et au-delà, retournée ou tournée d'un quart de tour aussi ; lissée dès qu'elle réduit.
    for (zoom, rotation, retournee, attendu) in [(1.0, 0.0, false, true), (4.0, 0.0, false, true), (1.0, 0.0, true, true), (1.0, quart, false, true), (2.0, -quart, true, true), (0.995, 0.0, false, true), (0.98, 0.0, false, false), (0.367, 0.0, false, false), (0.05, 0.0, true, false)] {
        assert_eq!(nette(zoom, rotation, retournee, false, 1.0), attendu, "vue droite, zoom {zoom}, tournée de {rotation}, retournée {retournee}");
    }
    // Vue tournée : lissée jusqu'à 102 %, nette au-delà (la vue nette, ou les pixels d'un dessin bitmap grossi).
    for (zoom, attendu) in [(0.367, false), (1.0, false), (1.01, false), (1.5, true), (4.0, true)] {
        assert_eq!(nette(zoom, 0.3, false, false, 1.0), attendu, "vue tournée, zoom {zoom}");
    }
    // En perspective : toujours lissée.
    assert!(!nette(2.0, 0.0, false, true, 1.0));
    // L'échelle compte en pixels de l'écran : à 1,5 pixel par point, 67 % en points font 100 % en pixels.
    assert!(nette(1.0 / 1.5, 0.0, false, false, 1.5));
    assert!(!nette(0.6, 0.0, false, false, 1.5));
}

/// Des images à regarder (essai ignoré), dans le dossier `FEANOR_CAPTURE_DIR` : la vue à 37 %, à 80 %, et à 37 % tournée, sur un plan
/// d'une COPIE de projet (`FEANOR_PROJET_ESSAI` : son manifeste ; `FEANOR_PLAN_ESSAI` : le numéro du plan, le dernier sinon) ou, sans
/// projet, sur la feuille de traits fins. Sans carte graphique : rien.
/// `FEANOR_CAPTURE_DIR=… FEANOR_PROJET_ESSAI=… cargo test --release --offline --target-dir target\msvc vue_reduite_captures -- --ignored --nocapture`
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn vue_reduite_captures() {
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let doc = match std::env::var_os("FEANOR_PROJET_ESSAI") {
        Some(p) => crate::project::load(std::path::Path::new(&p)).expect("le projet s'ouvre").doc,
        None => feuille_de_traits_fins(),
    };
    let plans = doc.panels.len().max(1);
    let plan = std::env::var("FEANOR_PLAN_ESSAI").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(plans).clamp(1, plans) - 1;
    let mut b = crate::perf_tests::banc(doc, true);
    if b.capturer().is_none() {
        return println!("CAPTURE pas de carte graphique : rien");
    }
    b.app.set_mode(crate::app::Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (plan, 0, 0);
    b.app.layout.safe_frames = false;
    b.chauffer(10);
    for (nom, zoom, rotation) in [("vue-37.png", 0.367_f32, 0.0_f32), ("vue-80.png", 0.8, 0.0), ("vue-37-tournee.png", 0.367, 0.3)] {
        poser_la_vue(&mut b, zoom, rotation);
        b.attendre_vignettes();
        b.chauffer(6);
        let rgba = b.capturer().expect("carte graphique");
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("CAPTURE {nom} : plan {}, zone du dessin {:?}", plan + 1, b.app.canvas_rect);
    }
}

// ---------------------------------------------------------------------------------------------- les vignettes (processeur)

/// L'encre d'un pixel opaque d'une vignette : la part couverte de noir, en lumière linéaire ou en valeurs gamma.
fn encre_de(c: eframe::egui::Color32, lineaire: bool) -> f32 {
    if lineaire {
        1.0 - eframe::egui::ecolor::linear_f32_from_gamma_u8(c.r())
    } else {
        1.0 - c.r() as f32 / 255.0
    }
}

/// Pour chaque trait et chaque ligne de la vignette qu'il traverse : l'encre de la ligne autour du trait, rapportée à ce qu'elle doit
/// être (un trait d'un pixel de la feuille, réduit `reduction` fois, laisse `1 / reduction` par ligne). `marge` : celle de la vignette.
fn encre_par_ligne_de_vignette(image: &eframe::egui::ColorImage, reduction: f32, marge: usize, lineaire: bool) -> Vec<f32> {
    let [l, h] = image.size;
    let mut mesures = Vec::new();
    for (x0, pente) in TRAITS {
        for yy in marge..h - marge {
            let y = (yy - marge) as f32 * reduction + reduction / 2.0;
            if y < HAUT as f32 + 12.0 || y > BAS as f32 - 12.0 {
                continue;
            }
            let x = (x0 as f32 + (y - HAUT as f32) / pente as f32 + 0.5) / reduction + marge as f32;
            let (a, z) = ((x - 5.0).round() as usize, ((x + 5.0).round() as usize).min(l - 1));
            mesures.push((a..=z).map(|xx| encre_de(image.pixels[yy * l + xx], lineaire)).sum::<f32>() * reduction);
        }
    }
    mesures
}

/// Une vignette garde les traits fins entiers, comme l'affichage : à chaque ligne, l'encre d'un trait d'un pixel vaut ce qu'elle doit, que
/// la vignette soit une case des Vignettes (avec sa marge, en lumière linéaire) ou une vignette de plan (sans marge, en gamma), au tiers
/// comme au vingtième. (Avant la partie 166.10 : 4 à 16 points lus par pixel ; un trait y perdait des lignes entières.)
#[test]
fn une_vignette_garde_les_traits_fins_entiers() {
    let doc = feuille_de_traits_fins();
    let plan = crate::render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
    let cam = doc.panel_camera(0, 0);
    for (largeur, marge) in [(592usize, 8usize), (300, 8), (300, 0), (160, 0), (80, 0)] {
        let hauteur = largeur * HAUTEUR / LARGEUR;
        let reduction = LARGEUR as f32 / largeur as f32;
        let image = crate::render::thumbnail_marge_until(&doc, &plan, &cam, largeur, hauteur, marge, None, &|| false).expect("jamais interrompue");
        assert_eq!(image.size, [largeur + 2 * marge, hauteur + 2 * marge]);
        let (moins, plus, moyenne, n) = resume(&encre_par_ligne_de_vignette(&image, reduction, marge, marge > 0));
        println!("     vignette de {largeur} pixels (réduction {reduction:.1}, marge {marge}) : {n} lignes, encre de {moins:.2} à {plus:.2} fois la normale, {moyenne:.3} en moyenne");
        assert!(n >= 100, "les traits sont dans la vignette : {n} lignes");
        assert!((moyenne - 1.0).abs() < 0.05 && moins >= 0.85 && plus <= 1.15, "vignette de {largeur} pixels : un trait fin garde son encre à chaque ligne, de {moins:.2} à {plus:.2} fois la normale ({moyenne:.3} en moyenne)");
    }
}

/// Le raccourci des pixels sans encre ne change aucun pixel : la même vignette avec et sans, caméra droite, caméra qui zoome et se
/// décale, caméra en roulis (lectures en grille), calque déplacé et tourné, avec une marge et sans.
#[test]
fn le_raccourci_des_pixels_sans_encre_ne_change_rien() {
    let mut doc = feuille_de_traits_fins();
    // (Un second calque, transformé : ses lectures sont lissées, le raccourci doit en tenir compte.)
    let dessin = doc.panels[0].layers[0].cel_at(0).expect("le calque a un dessin");
    let mut calque = Layer::new("Tourné", LayerKind::Bitmap);
    calque.set(0, Some(dessin));
    calque.transform = crate::document::LayerTransform { x: 37.5, y: -22.0, angle: 12.0, scale: 0.8, ..Default::default() };
    doc.panels[0].layers.push(calque);
    let plan = crate::render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
    let droite = doc.panel_camera(0, 0);
    let cameras = [droite, crate::camera::Camera { x: droite.x + 130.3, y: droite.y - 71.7, zoom: 1.7, ..droite }, crate::camera::Camera { angle: 17.0, zoom: 0.8, ..droite }];
    for (k, cam) in cameras.iter().enumerate() {
        for (largeur, marge) in [(300usize, 12usize), (173, 0)] {
            let hauteur = largeur * HAUTEUR / LARGEUR;
            let vignette = || crate::render::thumbnail_marge_until(&doc, &plan, cam, largeur, hauteur, marge, None, &|| false).expect("jamais interrompue");
            let avec = vignette();
            crate::render::SANS_RACCOURCI.with(|c| c.set(true));
            let sans = vignette();
            crate::render::SANS_RACCOURCI.with(|c| c.set(false));
            let differents = avec.pixels.iter().zip(&sans.pixels).filter(|(a, b)| a != b).count();
            assert_eq!(differents, 0, "caméra {k}, vignette de {largeur} pixels, marge {marge} : {differents} pixels diffèrent avec le raccourci");
            // (Un trait d'un pixel réduit cinq fois est pâle : 19 % d'encre, en lumière linéaire, donne un rouge vers 232.)
            assert!(avec.pixels.iter().any(|c| c.r() < 250 && c.a() > 0), "caméra {k} : la vignette montre de l'encre");
        }
    }
}

/// Au demi, caméra droite, sans marge : chaque pixel de la vignette est la moyenne de ses quatre pixels de la feuille, tronquée, comme
/// les quatre lectures d'avant.
#[test]
fn au_demi_la_vignette_est_la_moyenne_des_quatre_pixels() {
    let doc = feuille_de_traits_fins();
    let plan = crate::render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
    let cam = doc.panel_camera(0, 0);
    let image = crate::render::thumbnail(&doc, &plan, &cam, LARGEUR / 2, HAUTEUR / 2);
    for (x0, pente) in TRAITS {
        for y in (HAUT + 10..BAS - 10).step_by(7) {
            let x = colonne(x0, pente, y);
            let (vx, vy) = (x / 2, y / 2);
            // Les quatre pixels de la feuille sous ce pixel de vignette : papier (255), sauf ceux du trait (0).
            let noirs = [(2 * vx, 2 * vy), (2 * vx + 1, 2 * vy), (2 * vx, 2 * vy + 1), (2 * vx + 1, 2 * vy + 1)].iter().filter(|(px, py)| colonne(x0, pente, *py) == *px).count();
            let attendu = (255 * (4 - noirs) / 4) as u8;
            assert_eq!(image.pixels[vy * (LARGEUR / 2) + vx].r(), attendu, "pixel ({vx}, {vy}) de la vignette au demi : {noirs} pixels noirs sur quatre");
        }
    }
}
