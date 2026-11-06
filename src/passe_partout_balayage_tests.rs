//! Essais du passe-partout de la planche des Vignettes pour les plans à mouvement de caméra (2026-11-06) : l'ouverture est tout ce que la
//! caméra balaie, dans son vrai dessin, à l'écran et à l'export ; le numéro de plan, la touche qui le cache, la flèche.

use eframe::egui::{epaint::Shape, pos2, vec2, Color32, Pos2, Rect};

use crate::camera::{Camera, CameraKey};
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::{se_reposer, Banc};
use crate::planche::{self, Options};
use crate::raster::TILE;
use crate::vignettecam::{boite, dans_polygone, distance_bord};
use crate::vignettes_tests::{ouvert, quatre_plans};

const ROUGE: Color32 = Color32::from_rgb(255, 0, 0);

/// Quatre plans ; le plan 2 (la case 3 de la planche) a une caméra qui va de A (en haut à gauche de la feuille, serrée) à B (en bas à droite,
/// serrée, tournée), avec un rectangle rouge au milieu du trajet (ni dans A ni dans B) et un autre loin de tout, hors de l'enveloppe.
fn film_mobile() -> Document {
    let mut doc = quatre_plans();
    let n = doc.panels[2].frame_count();
    let a = Camera { x: 70.0, y: 50.0, zoom: 2.0, ..Camera::centered(doc.width, doc.height) };
    let b = Camera { x: 250.0, y: 130.0, zoom: 2.0, angle: 30.0, ..a };
    doc.panels[2].camera = vec![CameraKey::new(0, a), CameraKey::new(n - 1, b)];
    let cel = doc.new_cel(LayerKind::Bitmap);
    for (x0, y0, x1, y1) in [(148, 81, 172, 99), (30, 130, 50, 150)] {
        let r = &mut doc.cels.get_mut(&cel).expect("dessin").raster;
        for y in y0..y1 {
            for x in x0..x1 {
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = ROUGE;
            }
        }
    }
    let mut layer = Layer::new("Au milieu", LayerKind::Bitmap);
    layer.frames = vec![Some(cel); n];
    doc.panels[2].layers.push(layer);
    doc
}

fn rouge(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 60 && p[2] < 60
}

/// Combien de pixels rouges autour de (x, y), à `r` pixels près (la flèche bleue peut passer sur le dessin : un seul pixel ne dit rien).
fn rouges(img: &image::RgbImage, (x, y): (u32, u32), r: u32) -> usize {
    let (x0, y0) = (x.saturating_sub(r), y.saturating_sub(r));
    (y0..=(y + r).min(img.height() - 1)).flat_map(|j| (x0..=(x + r).min(img.width() - 1)).map(move |i| (i, j))).filter(|&(i, j)| rouge(img.get_pixel(i, j).0)).count()
}

/// Le point de la feuille `s`, sur la planche exportée, en pixels (la tuile du plan 2 le place : centre de la tuile = centre de sa zone).
fn pixel_de(p: &planche::Planche, s: f32, feuille: Pos2) -> (u32, u32) {
    let mv = p.cartes.iter().find_map(|c| c.mouvement.as_ref()).expect("le plan à mouvement");
    let z = mv.tuile.zone;
    let planche_pt = mv.tuile.rect.center() + (feuille - z.centre) * z.echelle;
    (((planche_pt.x - p.cadre.left()) * s).round() as u32, ((planche_pt.y - p.cadre.top()) * s).round() as u32)
}

fn exporter(b: &mut Banc, opt: &Options, nom: &str) -> image::RgbImage {
    let dir = std::env::temp_dir().join(format!("shot-test-balayage-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier");
    let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let sortie = dir.join("planche.png");
    b.app.export_job = None;
    let job = b.app.planche_job(&plans, opt, &sortie).expect("tâche");
    b.app.start_job(job);
    let debut = std::time::Instant::now();
    while b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) && debut.elapsed() < std::time::Duration::from_secs(120) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    b.chauffer(2);
    let fin = b.app.export_job.as_ref().and_then(|j| j.outcome.clone()).expect("fini");
    assert!(fin.is_ok(), "{fin:?}");
    let img = image::open(&sortie).expect("PNG").to_rgb8();
    std::fs::remove_dir_all(&dir).ok();
    img
}

// ------------------------------------------------------------------------------ la planche

#[test]
fn only_the_moving_plan_has_a_swept_zone_and_it_stays_inside_its_envelope() {
    let b = ouvert(film_mobile(), 3);
    let layout = b.app.board_layout();
    assert_eq!(layout.moves.len(), 1);
    let mv = &layout.moves[0];
    assert!(!mv.balayage.is_empty());
    for piece in &mv.balayage {
        for q in piece {
            assert!(dans_polygone(&mv.enveloppe, *q) || distance_bord(&mv.enveloppe, *q) < 1e-2, "{q:?} sort de l'enveloppe");
        }
    }
    // Les cadres A et B sont ouverts en entier ; l'entre-deux aussi.
    for c in &mv.cadres {
        assert!(mv.balayage.iter().any(|p| dans_polygone(p, c.rect.center())), "le cadre {} est ouvert", c.rang);
    }
    let milieu = mv.cadres[0].rect.center().lerp(mv.cadres[1].rect.center(), 0.5);
    assert!(mv.balayage.iter().any(|p| dans_polygone(p, milieu)), "l'entre-deux est ouvert");
    // Un plan sans mouvement n'a pas d'empreinte, donc rien de balayé.
    let sans = ouvert(quatre_plans(), 3);
    assert!(sans.app.board_layout().moves.is_empty());
}

fn noirs(b: &Banc) -> (Vec<Rect>, Vec<[Pos2; 3]>) {
    let (mut rects, mut tris) = (Vec::new(), Vec::new());
    for s in &b.formes {
        match &s.shape {
            Shape::Rect(r) if r.fill == Color32::BLACK => rects.push(r.rect),
            Shape::Mesh(m) if m.vertices.first().is_some_and(|v| v.color == Color32::BLACK) => {
                for t in m.indices.chunks_exact(3) {
                    tris.push([m.vertices[t[0] as usize].pos, m.vertices[t[1] as usize].pos, m.vertices[t[2] as usize].pos]);
                }
            }
            _ => {}
        }
    }
    (rects, tris)
}

fn dans_triangle(t: &[Pos2; 3], p: Pos2) -> bool {
    dans_polygone(t, p)
}

#[test]
fn the_screen_passe_partout_opens_on_everything_the_camera_sweeps_pixel_by_pixel() {
    let mut b = ouvert(film_mobile(), 3);
    b.garder_formes = true;
    b.app.run_action("vignettes.marie_louise");
    b.chauffer(3);
    assert!(b.app.vignettes.annot.marie);
    let layout = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let vue = b.app.vignettes.view;
    let (rects, tris) = noirs(&b);
    assert!(!tris.is_empty(), "un plan à mouvement : le passe-partout est un maillage découpé autour de la zone balayée");
    let couvert = |p: Pos2| rects.iter().any(|r| r.contains(p)) || tris.iter().any(|t| dans_triangle(t, p));
    // Les trous : les cases et les polygones balayés, à l'écran.
    let cases: Vec<Rect> = layout.cards.iter().map(|c| xf.rect(c.thumb)).collect();
    let pieces: Vec<Vec<Pos2>> = layout.moves.iter().flat_map(|m| m.balayage.iter()).map(|p| p.iter().map(|q| xf.to_screen(*q)).collect()).collect();
    let (mut ouverts, mut couverts, mut faux) = (0, 0, 0);
    let mut p = vue.left_top() + vec2(2.0, 2.0);
    while p.y < vue.bottom() - 2.0 {
        p.x = vue.left() + 2.0;
        while p.x < vue.right() - 2.0 {
            let trou = cases.iter().any(|c| c.contains(p)) || pieces.iter().any(|q| dans_polygone(q, p));
            let bord = cases.iter().map(|c| distance_bord(&[c.left_top(), c.right_top(), c.right_bottom(), c.left_bottom()], p)).chain(pieces.iter().map(|q| distance_bord(q, p))).fold(f32::MAX, f32::min);
            if bord > 1.5 {
                if trou == couvert(p) {
                    faux += 1;
                }
                if trou { ouverts += 1 } else { couverts += 1 }
            }
            p.x += 3.0;
        }
        p.y += 3.0;
    }
    assert!(ouverts > 500 && couverts > 5000, "{ouverts} ouverts, {couverts} couverts");
    assert_eq!(faux, 0, "des points sont couverts alors qu'ils sont dans ce que la caméra balaie, ou ouverts alors qu'ils n'en sont pas");
    // Les deux cadres, A et B, et la case : ouverts à leur centre.
    let mv = &layout.moves[0];
    for c in &mv.cadres {
        assert!(!couvert(xf.to_screen(c.rect.center())), "le cadre {} est ouvert", c.rang);
    }
    assert!(!couvert(xf.to_screen(mv.cadres[0].rect.center().lerp(mv.cadres[1].rect.center(), 0.5))), "l'entre-deux est ouvert");
    // Éteint, plus rien de noir.
    b.app.run_action("vignettes.marie_louise");
    b.chauffer(3);
    let (r2, t2) = noirs(&b);
    assert!(t2.is_empty() && r2.len() <= rects.len(), "éteint : plus de passe-partout");
}

#[test]
fn the_exported_board_shows_the_real_drawing_in_the_swept_zone_and_hides_the_rest() {
    let mut b = ouvert(film_mobile(), 3);
    let tous: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let opt = Options { marie: true, marie_opacite: 100, cadres: true, ..Options::default() };
    let p = planche::construire(&b.app.doc, &tous, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
    let s = planche::dimensions(p.cadre, opt.resolution).0;
    let mv = p.cartes.iter().find_map(|c| c.mouvement.as_ref()).expect("le plan à mouvement");
    let (milieu, loin) = ((160.0, 90.0), (40.0, 140.0));
    let (mx, my) = pixel_de(&p, s, pos2(milieu.0, milieu.1));
    let (lx, ly) = pixel_de(&p, s, pos2(loin.0, loin.1));
    // Hors de l'enveloppe : le point « loin » est bien sur la tuile, et dehors.
    let loin_planche = mv.tuile.rect.center() + (pos2(loin.0, loin.1) - mv.tuile.zone.centre) * mv.tuile.zone.echelle;
    assert!(mv.tuile.rect.contains(loin_planche) && !dans_polygone(&mv.enveloppe, loin_planche), "{loin_planche:?}");
    assert!(mv.balayage.iter().any(|q| dans_polygone(q, mv.tuile.rect.center())) || boite(&mv.enveloppe).intersects(mv.tuile.rect));
    // Avec le passe-partout : le rectangle du milieu du trajet se voit (dans son vrai dessin), l'autre, hors de ce que la caméra balaie, est masqué.
    let avec = exporter(&mut b, &opt, "avec");
    assert!(rouges(&avec, (mx, my), 8) > 30, "le dessin du milieu du trajet se voit : {} pixels rouges", rouges(&avec, (mx, my), 8));
    assert_eq!(rouges(&avec, (lx, ly), 8), 0, "le dessin hors de la zone balayée est masqué");
    assert_eq!(avec.get_pixel(lx, ly).0, [0, 0, 0], "et couvert par le passe-partout");
    // Sans le passe-partout : le dessin du milieu est dans l'export aussi (la tuile), et celui du dehors reste visible (il déborde).
    let sans = exporter(&mut b, &Options { marie: false, ..opt.clone() }, "sans");
    assert!(rouges(&sans, (mx, my), 8) > 30, "sans passe-partout : le dessin de la zone balayée est exporté");
    assert!(rouges(&sans, (lx, ly), 8) > 10, "sans passe-partout : le trait qui déborde est exporté");
    // Les cadres et la flèche : du bleu fin, la flèche plus pâle que le trait des cadres.
    let bleu = (0..avec.width()).step_by(1).any(|x| (0..avec.height()).step_by(7).any(|y| { let q = avec.get_pixel(x, y).0; q[2] > 200 && q[0] < 120 }));
    assert!(bleu, "les cadres bleus sont dans l'export");
    assert!(se_reposer(&mut b, 80) < 80, "l'écran se repose");
}

#[test]
fn rotation_and_zoom_enter_the_export_opening_too() {
    let b = ouvert(film_mobile(), 3);
    let tous: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let opt = Options { marie: true, cadres: false, ..Options::default() };
    let p = planche::construire(&b.app.doc, &tous, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
    // Même sans les cadres, l'ouverture sait ce que la caméra balaie (la rotation de B comprise).
    let mv = p.cartes.iter().find_map(|c| c.mouvement.as_ref()).expect("avec le passe-partout, même sans les cadres");
    assert!(mv.balayage.len() >= 2, "une rotation : plusieurs pièces ({})", mv.balayage.len());
    let sans_marie = planche::construire(&b.app.doc, &tous, &b.app.layout.board_settings(), &Options { marie: false, cadres: false, ..Options::default() }, &b.app.doc.annotations);
    assert!(sans_marie.cartes.iter().all(|c| c.mouvement.is_none()), "ni cadres ni passe-partout : rien à dessiner");
}

// ------------------------------------------------------------------------------ rien ne change dans les données

#[test]
fn undo_and_save_leave_the_drawing_data_alone() {
    let mut b = ouvert(film_mobile(), 3);
    let avant: Vec<(usize, usize, Vec<CameraKey>)> = b.app.doc.panels.iter().map(|p| (p.id as usize, p.frame_count(), p.camera.clone())).collect();
    let (cels, etapes) = (b.app.doc.cels.len(), b.app.history.undo_len());
    b.app.run_action("vignettes.marie_louise");
    b.app.run_action("vignettes.numeros");
    b.app.run_action("vignettes.numeros");
    b.chauffer(3);
    let apres: Vec<(usize, usize, Vec<CameraKey>)> = b.app.doc.panels.iter().map(|p| (p.id as usize, p.frame_count(), p.camera.clone())).collect();
    assert_eq!(avant, apres, "les caméras et les plans n'ont pas bougé");
    assert_eq!((b.app.doc.cels.len(), b.app.history.undo_len()), (cels, etapes), "aucune étape d'annulation, aucun dessin de plus");
    // L'export non plus.
    let tous: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let _ = exporter(&mut b, &Options { marie: true, ..Options::default() }, "donnees");
    assert_eq!((b.app.doc.cels.len(), b.app.history.undo_len()), (cels, etapes));
    assert_eq!(tous.len(), 4);
}

// ------------------------------------------------------------------------------ les numéros

#[test]
fn the_number_key_is_a_free_simple_key_remembered_in_the_settings_and_it_acts_on_the_export() {
    let mut b = ouvert(quatre_plans(), 3);
    assert!(b.app.settings.vignettes.numeros_visibles, "montrés d'origine");
    let touche = crate::keymap::ACTIONS.iter().find(|a| a.id == "vignettes.numeros").expect("l'action est nommée");
    assert!(touche.implemented && touche.label.contains("numéros"));
    let toml = include_str!("../keymap.toml");
    assert!(toml.contains("action = \"vignettes.numeros\"\ntouches = [\"N\"]"), "la touche N dans keymap.toml");
    b.app.run_action("vignettes.numeros");
    assert!(!b.app.settings.vignettes.numeros_visibles && b.app.settings_a_ecrire, "caché, et à écrire dans les réglages de l'application");
    let t = toml::to_string(&b.app.settings.vignettes).expect("réglages");
    assert!(t.contains("numeros_visibles = false"), "{t}");
    let relu: crate::vignettesreglages::ReglagesVignettes = toml::from_str(&t).expect("relus");
    assert!(!relu.numeros_visibles);
    let ancien: crate::vignettesreglages::ReglagesVignettes = toml::from_str("debord_pour_cent = 10").expect("anciens réglages");
    assert!(ancien.numeros_visibles, "un réglage d'avant : montrés");
    // L'export suit, comme les textes : cachés à l'écran, cachés dans l'export (la case de la fenêtre peut aussi les cacher).
    let (opt, _) = b.app.planche_options_de_depart();
    assert!(!b.app.planche_options(&Options { numeros: true, ..opt.clone() }).numeros);
    b.app.run_action("vignettes.numeros");
    assert!(b.app.planche_options(&Options { numeros: true, ..opt.clone() }).numeros);
    assert!(!b.app.planche_options(&Options { numeros: false, ..opt }).numeros, "la case de la fenêtre décoche aussi");
    // Même touche sur la palette : l'action est cherchable par son nom.
    assert!(crate::keymap::ACTIONS.iter().any(|a| a.label.to_lowercase().contains("numéros de plan")));
}

#[test]
fn hidden_numbers_are_not_painted_and_shown_ones_never_cover_an_image_a_frame_or_a_swept_zone() {
    use eframe::egui::epaint::Shape;
    let mut b = ouvert(film_mobile(), 3);
    b.garder_textes = true;
    b.garder_formes = true;
    b.chauffer(3);
    let layout = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    // Le numéro d'un plan se retrouve par son texte et sa place.
    let numeros = |b: &Banc| -> Vec<(String, Rect)> {
        b.formes
            .iter()
            .filter_map(|s| if let Shape::Text(t) = &s.shape { Some((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))) } else { None })
            .filter(|(t, r)| t.chars().all(|c| c.is_ascii_digit()) && !t.is_empty() && r.height() > 20.0)
            .collect()
    };
    let montres = numeros(&b);
    let etiquettes: std::collections::BTreeSet<&str> = montres.iter().map(|(t, _)| t.as_str()).collect();
    assert!(etiquettes.len() >= 3, "les numéros des plans qu'on voit : {montres:?}");
    let mut obstacles: Vec<(Rect, Option<Vec<Pos2>>)> = layout.cards.iter().map(|c| (xf.rect(c.thumb), None)).collect();
    obstacles.extend(layout.moves.iter().map(|m| (Rect::NOTHING, Some(m.enveloppe.iter().map(|q| xf.to_screen(*q)).collect::<Vec<_>>()))));
    for (t, r) in &montres {
        for (rect, poly) in &obstacles {
            let touche = match poly {
                None => r.shrink(0.6).intersects(rect.shrink(0.6)),
                Some(p) => {
                    let coins = [r.shrink(0.6).left_top(), r.shrink(0.6).right_top(), r.shrink(0.6).right_bottom(), r.shrink(0.6).left_bottom()];
                    coins.iter().any(|c| dans_polygone(p, *c)) || p.iter().any(|q| r.shrink(0.6).contains(*q))
                }
            };
            assert!(!touche, "le numéro {t} ({r:?}) recouvre une image, un cadre ou une zone balayée");
        }
    }
    // Caché : les numéros des plans ne sont plus peints.
    b.app.run_action("vignettes.numeros");
    b.chauffer(3);
    assert!(numeros(&b).is_empty(), "numéros cachés : {:?}", numeros(&b));
    b.app.run_action("vignettes.numeros");
    b.chauffer(3);
    assert_eq!(numeros(&b).len(), montres.len(), "remontrés");
}

#[test]
fn the_number_stays_attached_to_frame_a_of_a_moving_plan_when_its_margin_is_free() {
    let b = ouvert(film_mobile(), 3);
    let layout = b.app.board_layout();
    let k = layout.cards.iter().position(|c| c.mv.is_some()).expect("le plan à mouvement");
    let xf = b.app.vignettes.xf;
    let a = xf.rect(layout.cards[k].thumb);
    let mut marges = crate::numerosmarge::Marges::pour(&layout, xf, b.app.vignettes.view);
    let (r, _) = marges.choisir(a, a.top(), |sc| vec2(30.0, 18.0) * sc).expect("une place");
    // Soit à son coin d'origine, soit (la zone balayée prend la marge) tout près de A : jamais loin de lui.
    assert!(r.center().distance(a.center()) < a.width() * 1.2 + 4.0 * 18.0, "{r:?} loin de A {a:?}");
}
