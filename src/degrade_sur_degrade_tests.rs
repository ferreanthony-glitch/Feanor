//! Un dégradé posé sur un autre dégradé (Anthony, 2026-10-28 : « Le dégradé sur un autre dégradé crée ce genre de bug »).
//!
//! Le bug : sans sélection, l'outil Dégradé cherche la zone fermée où l'on appuie comme le fait le pot de peinture, c'est-à-dire
//! les pixels qui ont la couleur du point d'appui, à une tolérance près. Sur un dégradé déjà posé, aucune couleur n'est unie :
//! la « zone » n'était qu'une bande de couleurs voisines, coupée d'une limite dure qui n'a rien à voir avec le dessin.
//! La règle : la zone n'est bornée que par le trait (un saut de couleur), pas par une variation douce.

use std::time::Duration;

use eframe::egui::{pos2, Color32, Modifiers};

use crate::app::Tool;
use crate::degrade::{self, Degrade, Reglages};
use crate::degrade_tests::{anneau, app, banc_degrade, cel_du_calque, etapes, glisser, pixel, tracer_anneau, BLEU, ROUGE};
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::perf_tests::Banc;
use crate::pixsel::PixelMask;

/// Un gris moyen et un gris clair : de quoi contraster avec le noir du trait (un dégradé qui part du noir collerait au trait noir,
/// ce que le pot de peinture fait aussi : même couleur, même zone).
const SOMBRE: [u8; 3] = [40, 40, 40];
const CLAIR: [u8; 3] = [200, 200, 200];

/// Pose un dégradé de `avant` à `arriere` entre deux points de la feuille, avec l'outil Dégradé.
fn poser(b: &mut Banc, avant: [u8; 3], arriere: [u8; 3], de: (f32, f32), vers: (f32, f32), touches: Modifiers) {
    b.app.set_tool(Tool::Gradient);
    b.app.color = avant;
    b.app.couleurs.prefs.back = arriere;
    glisser(b, &[de, vers], touches);
}

/// Les pixels qui ne sont pas du côté voulu d'un dégradé rouge → bleu court (axe de x = `a` à x = `c`) : rouge avant `a - 8`,
/// bleu après `c + 8`. Rend (nombre de pixels faux, le premier).
fn faux_rouge_bleu(b: &Banc, x0: usize, x1: usize, y0: usize, y1: usize, a: usize, c: usize) -> (usize, Option<(usize, usize, Color32)>) {
    let (mut n, mut premier) = (0, None);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let p = pixel(b, x, y);
            let bon = if x + 8 <= a {
                p.a() == 255 && p.r() >= 245 && p.b() <= 10
            } else if x >= c + 8 {
                p.a() == 255 && p.b() >= 245 && p.r() <= 10
            } else {
                true
            };
            if !bon {
                n += 1;
                premier.get_or_insert((x, y, p));
            }
        }
    }
    (n, premier)
}

// ------------------------------------------------------------------------------------ bitmap

#[test]
fn a_gradient_over_a_gradient_replaces_the_whole_area_of_the_first_without_a_hard_edge() {
    let mut b = banc_degrade(LayerKind::Bitmap);
    // A : sur le calque vide, sans sélection : la zone est toute la feuille ; un axe en diagonale.
    poser(&mut b, SOMBRE, CLAIR, (20.0, 20.0), (380.0, 280.0), Modifiers::NONE);
    for (x, y) in [(0, 0), (399, 0), (0, 299), (399, 299), (200, 150)] {
        assert_eq!(pixel(&b, x, y).a(), 255, "A couvre la feuille en ({x}, {y})");
    }
    let avant = etapes(&b.app).len();
    // B : un axe court, au milieu, par-dessus A.
    poser(&mut b, ROUGE, BLEU, (200.0, 100.0), (230.0, 100.0), Modifiers::NONE);
    assert_eq!(etapes(&b.app).len(), avant + 1, "une seule étape d'annulation");
    let (n, premier) = faux_rouge_bleu(&b, 0, 399, 0, 299, 200, 230);
    assert_eq!(n, 0, "B couvre toute la zone de A : il reste {n} pixels de l'ancien dégradé, le premier {premier:?}");
    // Annuler d'un coup : A est revenu.
    b.app.undo();
    let p = pixel(&b, 5, 5);
    assert!(p.r() < 80 && p.r() == p.g(), "A est revenu : {p:?}");
}

#[test]
fn a_gradient_over_a_gradient_in_a_closed_shape_stays_in_the_shape() {
    let mut b = banc_degrade(LayerKind::Bitmap);
    tracer_anneau(&mut b);
    poser(&mut b, SOMBRE, CLAIR, (150.0, 150.0), (250.0, 190.0), Modifiers::NONE);
    assert_eq!(pixel(&b, 110, 110).a(), 255, "A remplit l'anneau");
    let avant = etapes(&b.app).len();
    poser(&mut b, ROUGE, BLEU, (200.0, 150.0), (230.0, 150.0), Modifiers::NONE);
    assert_eq!(etapes(&b.app).len(), avant + 1);
    // Tout l'intérieur (hors le bord du trait) est à B.
    let (n, premier) = faux_rouge_bleu(&b, 110, 290, 110, 190, 200, 230);
    assert_eq!(n, 0, "B remplace tout l'intérieur de l'anneau : {n} pixels de l'ancien dégradé, le premier {premier:?}");
    // Hors de l'anneau : rien ; le trait est resté.
    for (x, y) in [(50, 150), (200, 50), (350, 150), (200, 250)] {
        assert_eq!(pixel(&b, x, y).a(), 0, "hors de l'anneau, rien en ({x}, {y})");
    }
    let t = pixel(&b, 200, 100);
    assert!(t.a() == 255 && t.r() < 10, "le trait reste : {t:?}");
}

#[test]
fn a_gradient_over_a_gradient_with_a_selection_stays_in_the_selection() {
    let mut b = banc_degrade(LayerKind::Bitmap);
    poser(&mut b, SOMBRE, CLAIR, (20.0, 20.0), (380.0, 280.0), Modifiers::NONE);
    let (cel, _) = b.app.ensure_cel();
    let avant = b.app.doc.cels[&cel].raster.to_flat();
    let poly = vec![pos2(40.0, 40.5), pos2(160.0, 40.5), pos2(160.0, 90.0), pos2(40.0, 90.0)];
    let mask = PixelMask::from_polygons(&[poly], 400, 300).expect("une zone");
    b.app.bsel = Some(crate::pixtools::BSelection { cel, mask, outline: Vec::new(), lifted: None });
    b.chauffer(2);
    poser(&mut b, ROUGE, BLEU, (60.0, 60.0), (70.0, 60.0), Modifiers::NONE);
    let w = b.app.doc.width;
    let apres = b.app.doc.cels[&cel].raster.to_flat();
    for (x, y) in [(20, 60), (200, 150), (180, 60), (100, 120), (399, 299), (100, 38)] {
        assert_eq!(apres[y * w + x], avant[y * w + x], "hors de la sélection, A est intact en ({x}, {y})");
    }
    let (g, d) = (pixel(&b, 45, 70), pixel(&b, 150, 70));
    assert!(g.r() >= 245 && g.b() <= 10 && d.b() >= 245 && d.r() <= 10, "dans la sélection, B : {g:?} {d:?}");
}

#[test]
fn alt_puts_the_second_gradient_on_the_whole_layer() {
    let mut b = banc_degrade(LayerKind::Bitmap);
    tracer_anneau(&mut b);
    poser(&mut b, SOMBRE, CLAIR, (150.0, 150.0), (250.0, 190.0), Modifiers::NONE);
    poser(&mut b, ROUGE, BLEU, (200.0, 20.0), (230.0, 20.0), Modifiers::ALT);
    let (n, premier) = faux_rouge_bleu(&b, 0, 399, 0, 299, 200, 230);
    assert_eq!(n, 0, "Alt : tout le calque, anneau compris : {n} pixels faux, le premier {premier:?}");
}

#[test]
fn a_gradient_over_a_painted_gradient_stops_at_a_contrasting_line_drawn_over_it() {
    // Un dégradé posé sur toute la feuille, puis un trait noir tracé par-dessus : le second dégradé, appuyé d'un côté du trait,
    // ne passe pas de l'autre côté.
    let mut b = banc_degrade(LayerKind::Bitmap);
    poser(&mut b, SOMBRE, CLAIR, (20.0, 20.0), (380.0, 20.0), Modifiers::NONE);
    b.app.set_tool(Tool::Brush);
    b.app.color = [0, 0, 0];
    glisser(&mut b, &[(0.0, 100.0), (200.0, 100.0), (399.0, 100.0)], Modifiers::NONE);
    poser(&mut b, ROUGE, BLEU, (200.0, 50.0), (230.0, 50.0), Modifiers::NONE);
    let (n, premier) = faux_rouge_bleu(&b, 0, 399, 0, 90, 200, 230);
    assert_eq!(n, 0, "au-dessus du trait : tout est à B, {n} pixels faux, le premier {premier:?}");
    for (x, y) in [(10, 150), (200, 250), (390, 299)] {
        let p = pixel(&b, x, y);
        assert!(p.r() == p.g() && p.r() < 210, "sous le trait : l'ancien dégradé reste en ({x}, {y}) : {p:?}");
    }
}

// ------------------------------------------------------------------------------------ vectoriel

fn zones(b: &Banc) -> usize {
    let cel = cel_du_calque(b);
    b.app.drawing(cel).map_or(0, |d| d.elements.iter().filter(|e| matches!(e, crate::vector::Element::Fill { .. })).count())
}

#[test]
fn on_a_vector_layer_a_gradient_over_a_gradient_replaces_it_in_place() {
    let mut b = banc_degrade(LayerKind::Vector);
    poser(&mut b, SOMBRE, CLAIR, (20.0, 20.0), (380.0, 280.0), Modifiers::NONE);
    assert_eq!(zones(&b), 1, "A : une zone qui couvre la feuille");
    let avant = etapes(&b.app).len();
    poser(&mut b, ROUGE, BLEU, (200.0, 100.0), (230.0, 100.0), Modifiers::NONE);
    assert_eq!(zones(&b), 1, "B remplace le dégradé de A : pas de seconde zone");
    assert_eq!(etapes(&b.app).len(), avant + 1);
    let (n, premier) = faux_rouge_bleu(&b, 0, 399, 0, 299, 200, 230);
    assert_eq!(n, 0, "la feuille est à B : {n} pixels faux, le premier {premier:?}");
}

#[test]
fn on_a_vector_layer_a_gradient_over_a_gradient_in_a_closed_shape_stays_in_the_shape() {
    let mut b = banc_degrade(LayerKind::Vector);
    tracer_anneau(&mut b);
    poser(&mut b, SOMBRE, CLAIR, (150.0, 150.0), (250.0, 190.0), Modifiers::NONE);
    assert_eq!(zones(&b), 1);
    // Plusieurs points d'appui dans l'anneau, dont tout près du trait : chacun donne le même résultat.
    for (k, (x, y)) in [(200.0, 150.0), (112.0, 112.0), (288.0, 188.0), (114.0, 150.0)].into_iter().enumerate() {
        let avant = etapes(&b.app).len();
        poser(&mut b, ROUGE, BLEU, (x, y), (x + 30.0, y), Modifiers::NONE);
        assert_eq!(zones(&b), 1, "appui {k} : toujours une seule zone");
        assert_eq!(etapes(&b.app).len(), avant + 1, "appui {k} : une étape");
        // (Axe court de x à x + 30 : les pixels à gauche de x - 8 sont rouges, ceux à droite de x + 38 bleus.)
        let (n, premier) = faux_rouge_bleu(&b, 110, 290, 110, 190, x as usize, x as usize + 30);
        assert_eq!(n, 0, "appui {k} : tout l'intérieur est à B, {n} pixels faux, le premier {premier:?}");
        let cel = cel_du_calque(&b);
        let g = crate::vector::Element::gradient(&b.app.drawing(cel).unwrap().elements[0]).cloned().expect("un dégradé");
        assert!((g.depart[0] - x).abs() < 1.5 && g.c0() == ROUGE && g.c1() == BLEU, "appui {k} : le dégradé de B : {g:?}");
    }
    for (x, y) in [(50, 150), (200, 50), (350, 150)] {
        assert_eq!(pixel(&b, x, y).a(), 0, "hors de l'anneau, rien en ({x}, {y})");
    }
}

#[test]
fn on_a_vector_layer_alt_puts_the_second_gradient_on_top_of_the_first() {
    let mut b = banc_degrade(LayerKind::Vector);
    tracer_anneau(&mut b);
    poser(&mut b, SOMBRE, CLAIR, (150.0, 150.0), (250.0, 190.0), Modifiers::NONE);
    poser(&mut b, ROUGE, BLEU, (200.0, 20.0), (230.0, 20.0), Modifiers::ALT);
    // La feuille entière est à B, l'anneau (de A) compris : la zone neuve ne passe pas sous celle de A.
    let (n, premier) = faux_rouge_bleu(&b, 0, 399, 0, 90, 200, 230);
    assert_eq!(n, 0, "Alt en vectoriel : B couvre A, {n} pixels faux, le premier {premier:?}");
    let (n, premier) = faux_rouge_bleu(&b, 110, 290, 110, 190, 200, 230);
    assert_eq!(n, 0, "dans l'anneau aussi : {n} pixels faux, le premier {premier:?}");
}

// ------------------------------------------------------------------------------------ l'aplat et le calque de couleur

/// Un plan 400 × 300 dont le calque a l'anneau (60, 60)–(220, 200) ; l'aplat est la cible.
fn aplat_cible() -> crate::app::App {
    let mut app = app("sur-aplat");
    let mut doc = Document::new(400, 300, 24);
    let cel = doc.add_cel(Cel::bitmap(anneau(400, 300)));
    let mut ligne = Layer::new("Trait", LayerKind::Bitmap);
    ligne.frames = vec![Some(cel); 4];
    doc.panels[0].layers = vec![ligne];
    doc.panels[0].length = 4;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.toggle_fond(0);
    app.select_aplat(0);
    app.set_tool(Tool::Gradient);
    app
}

fn degrade_a_la_souris(app: &mut crate::app::App, avant: [u8; 3], arriere: [u8; 3], de: (f32, f32), vers: (f32, f32), alt: bool) {
    app.color = avant;
    app.couleurs.prefs.back = arriere;
    app.degrade_press(pos2(de.0, de.1), alt);
    assert!(app.degrade_geste.is_some(), "{}", app.status);
    app.degrade_drag(pos2(vers.0, vers.1), false);
    app.degrade_release();
}

#[test]
fn in_the_aplat_a_gradient_over_a_gradient_replaces_the_whole_zone() {
    let mut app = aplat_cible();
    let cel = app.layers()[0].frames[0].unwrap();
    degrade_a_la_souris(&mut app, SOMBRE, CLAIR, (70.0, 70.0), (210.0, 190.0), false);
    degrade_a_la_souris(&mut app, ROUGE, BLEU, (100.0, 130.0), (120.0, 130.0), false);
    let s = app.doc.cels[&cel].fond.as_ref().expect("la strate de l'aplat");
    let mut faux = Vec::new();
    for y in 70..=190 {
        for x in 70..=210 {
            let p = s.peinture.raster.get(x, y);
            let bon = if x <= 92 { p.a() == 255 && p.r() >= 245 && p.b() <= 10 } else if x >= 128 { p.a() == 255 && p.b() >= 245 && p.r() <= 10 } else { true };
            if !bon {
                faux.push((x, y, p));
            }
        }
    }
    assert!(faux.is_empty(), "l'aplat est tout à B : {} pixels faux, le premier {:?}", faux.len(), faux.first());
}

#[test]
fn on_a_colour_layer_a_gradient_over_a_gradient_replaces_the_whole_zone() {
    let mut app = app("sur-couleur");
    let mut doc = Document::new(400, 300, 24);
    let cel = doc.add_cel(Cel::bitmap(anneau(400, 300)));
    let mut ligne = Layer::new("Trait", LayerKind::Bitmap);
    ligne.frames = vec![Some(cel); 4];
    doc.panels[0].layers = vec![ligne, Layer::new("Autre", LayerKind::Vector)];
    doc.panels[0].length = 4;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.run_action("calque.nouveau_couleur");
    app.set_tool(Tool::Gradient);
    degrade_a_la_souris(&mut app, SOMBRE, CLAIR, (70.0, 70.0), (210.0, 190.0), false);
    degrade_a_la_souris(&mut app, ROUGE, BLEU, (100.0, 130.0), (120.0, 130.0), false);
    let id = app.layers()[0].frames[0].expect("le dessin de la couleur");
    let r = &app.doc.cels[&id].raster;
    let mut faux = 0;
    for y in 70..=190 {
        for x in 70..=210 {
            let p = r.get(x, y);
            let bon = if x <= 92 { p.a() == 255 && p.r() >= 245 && p.b() <= 10 } else if x >= 128 { p.a() == 255 && p.b() >= 245 && p.r() <= 10 } else { true };
            if !bon {
                faux += 1;
            }
        }
    }
    assert_eq!(faux, 0, "le calque de couleur est tout à B");
}

// ------------------------------------------------------------------------------------ le noyau

#[test]
fn the_gentle_zone_runs_through_a_smooth_gradient_and_stops_at_a_line_and_at_the_edge_of_the_drawing() {
    let (w, h) = (100usize, 50usize);
    let g: Degrade = Reglages::default().degrade([40, 40, 40], [200, 200, 200], pos2(10.0, 0.0), pos2(90.0, 0.0));
    let mut src = vec![Color32::TRANSPARENT; w * h];
    for y in 10..40 {
        for x in 10..90 {
            src[y * w + x] = g.pixel(x as f32 + 0.5, y as f32 + 0.5, 1.0);
        }
    }
    // Un trait noir de 2 px autour.
    for y in 8..42 {
        for x in 8..92 {
            if !(10..90).contains(&x) || !(10..40).contains(&y) {
                src[y * w + x] = Color32::BLACK;
            }
        }
    }
    let tol = 31u8;
    let classique = crate::fill::flood(&src, w, h, 50, 25, tol);
    assert!(classique.iter().filter(|&&b| b).count() < 30 * 80 / 2, "le pot de peinture ne prend qu'une bande de ce dégradé");
    let douce = degrade::zone_douce(&src, w, h, 50, 25, tol);
    for y in 0..h {
        for x in 0..w {
            let dedans = (10..90).contains(&x) && (10..40).contains(&y);
            assert_eq!(douce[y * w + x], dedans, "({x}, {y})");
        }
    }
    // Un point d'appui sur une couleur unie : le même résultat que le pot de peinture.
    let mut plat = vec![Color32::TRANSPARENT; w * h];
    for y in 8..42 {
        for x in 8..92 {
            plat[y * w + x] = if (10..90).contains(&x) && (10..40).contains(&y) { Color32::from_rgb(200, 50, 50) } else { Color32::BLACK };
        }
    }
    assert!(degrade::zone_douce(&plat, w, h, 50, 25, tol) == crate::fill::flood(&plat, w, h, 50, 25, tol));
    // Un point d'appui sur le vide : tout le vide connexe, comme le pot de peinture.
    assert!(degrade::zone_douce(&src, w, h, 3, 3, tol) == crate::fill::flood(&src, w, h, 3, 3, tol));
}

#[test]
fn the_gentle_zone_does_not_walk_along_the_soft_edge_of_a_slanted_line() {
    // Un trait presque horizontal, un peu penché, au bord anticrénelé : le long du bord, la couverture varie lentement d'un
    // pixel au suivant. Un dégradé sur toute la feuille, le trait noir par-dessus : le haut est séparé du bas.
    let (w, h) = (200usize, 100usize);
    let g = Reglages::default().degrade([40, 40, 40], [200, 200, 200], pos2(0.0, 0.0), pos2(199.0, 0.0));
    let mut src = vec![Color32::TRANSPARENT; w * h];
    let mut noyau = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let fond = g.pixel(x as f32 + 0.5, y as f32 + 0.5, 1.0);
            let axe = 48.0 + 0.04 * x as f32;
            let couverture = (3.0 - (y as f32 + 0.5 - axe).abs()).clamp(0.0, 1.0);
            noyau[y * w + x] = couverture >= 1.0;
            let k = 1.0 - couverture;
            src[y * w + x] = Color32::from_rgba_premultiplied((fond.r() as f32 * k).round() as u8, (fond.g() as f32 * k).round() as u8, (fond.b() as f32 * k).round() as u8, 255);
        }
    }
    let douce = degrade::zone_douce(&src, w, h, 100, 10, 31);
    let mut fautes = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let k = y * w + x;
            if noyau[k] && douce[k] {
                fautes.push(format!("le trait ({x}, {y})"));
            }
            if y >= 62 && douce[k] {
                fautes.push(format!("le bas ({x}, {y})"));
            }
            if y <= 40 && !douce[k] {
                fautes.push(format!("le haut manque ({x}, {y})"));
            }
        }
    }
    assert!(fautes.is_empty(), "{} pixels faux, les premiers : {:?}", fautes.len(), &fautes[..fautes.len().min(5)]);
}

#[test]
fn finding_the_gentle_zone_of_a_4k_sheet_is_a_single_quick_pass_at_the_press() {
    // Une feuille 4K, un dégradé sur toute la largeur, un cadre noir : la zone est tout l'intérieur, trouvée en un passage.
    let (w, h) = (3840usize, 2160usize);
    let g = Reglages::default().degrade([40, 40, 40], [255, 255, 255], pos2(100.0, 0.0), pos2(3740.0, 0.0));
    let mut src = vec![Color32::BLACK; w * h];
    for y in 100..2060 {
        for x in 100..3740 {
            src[y * w + x] = g.pixel(x as f32 + 0.5, y as f32 + 0.5, 1.0);
        }
    }
    let debut = std::time::Instant::now();
    let douce = degrade::zone_douce(&src, w, h, 1900, 1000, 31);
    let duree = debut.elapsed();
    let dedans = (100..2060).flat_map(|y| (100..3740).map(move |x| (x, y))).filter(|&(x, y)| douce[y * w + x]).count();
    assert!(dedans >= 1960 * 3640 - 4 * 1960, "tout l'intérieur du cadre : {dedans}");
    assert!(!douce[50 * w + 50] && !douce[2100 * w + 3800], "pas le cadre ni ce qui est autour");
    assert!(duree < Duration::from_millis(3000), "la zone d'une feuille 4K se trouve vite : {duree:?}");
}
